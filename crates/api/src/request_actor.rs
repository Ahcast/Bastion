//! Reuse an authenticated identity only within the current HTTP request.
//! Permissions, module state, quotas and CSRF remain checked by their owners.
use std::{future::Future, sync::Arc};

use axum::{extract::Request, middleware::Next, response::Response};
use bastion_core::{AuthenticatedUser, CoreError};
use tokio::sync::OnceCell;

struct RequestActor {
    token: Option<String>,
    user: OnceCell<AuthenticatedUser>,
}

impl RequestActor {
    fn new(token: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            token,
            user: OnceCell::new(),
        })
    }
}

tokio::task_local! {
    static CURRENT: Arc<RequestActor>;
}

pub(super) async fn scope(request: Request, next: Next) -> Response {
    // The key comes from the same cookie parser as actor(), never a client
    // identity header. scope() restores prior context on completion/cancellation.
    let cache = RequestActor::new(super::cookie_value(
        request.headers(),
        "__Host-bastion_session",
    ));
    CURRENT.scope(cache, next.run(request)).await
}

pub(super) async fn resolve<F, Fut>(token: &str, load: F) -> Result<AuthenticatedUser, CoreError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<AuthenticatedUser, CoreError>>,
{
    let cache = CURRENT.try_with(Arc::clone).ok();
    match cache {
        Some(cache) if cache.token.as_deref() == Some(token) => {
            cache.user.get_or_try_init(load).await.cloned()
        }
        // Background tasks do not inherit task locals; without this scope or
        // for another token, perform full authentication rather than reuse data.
        _ => load().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn user(id: u64) -> AuthenticatedUser {
        AuthenticatedUser {
            id,
            person_id: id,
            name: "Identity in memory".into(),
            email: "identity@example.invalid".into(),
            role: "Role".into(),
        }
    }

    #[tokio::test]
    async fn duplicate_guard_and_handler_calls_authenticate_once() {
        let calls = AtomicUsize::new(0);
        CURRENT
            .scope(RequestActor::new(Some("session".into())), async {
                let load = || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    Ok(user(1))
                };
                let (guard, handler) =
                    tokio::join!(resolve("session", load), resolve("session", load));
                assert_eq!(guard.unwrap().id, 1);
                assert_eq!(handler.unwrap().id, 1);
                assert_eq!(calls.load(Ordering::SeqCst), 1);
            })
            .await;
    }

    #[tokio::test]
    async fn new_request_rechecks_revocation_even_with_the_same_cookie() {
        CURRENT
            .scope(RequestActor::new(Some("session".into())), async {
                resolve("session", || async { Ok(user(1)) }).await.unwrap();
            })
            .await;
        CURRENT
            .scope(RequestActor::new(Some("session".into())), async {
                let result = resolve("session", || async { Err(CoreError::SessionReplaced) }).await;
                assert!(matches!(result, Err(CoreError::SessionReplaced)));
            })
            .await;
    }

    #[tokio::test]
    async fn different_tokens_and_concurrent_requests_never_share_identity() {
        let first = CURRENT.scope(RequestActor::new(Some("one".into())), async {
            resolve("one", || async { Ok(user(1)) }).await.unwrap();
            assert!(matches!(
                resolve("other", || async { Err(CoreError::Unauthorized) }).await,
                Err(CoreError::Unauthorized)
            ));
            tokio::task::yield_now().await;
            resolve("one", || async { Err(CoreError::Unauthorized) })
                .await
                .unwrap()
                .id
        });
        let second = CURRENT.scope(RequestActor::new(Some("two".into())), async {
            resolve("two", || async { Ok(user(2)) }).await.unwrap().id
        });
        assert_eq!(tokio::join!(first, second), (1, 2));
    }

    #[tokio::test]
    async fn failed_authentication_is_not_cached_and_unscoped_calls_fail_closed() {
        CURRENT
            .scope(RequestActor::new(Some("session".into())), async {
                assert!(
                    resolve("session", || async { Err(CoreError::Unauthorized) })
                        .await
                        .is_err()
                );
                assert_eq!(
                    resolve("session", || async { Ok(user(1)) })
                        .await
                        .unwrap()
                        .id,
                    1
                );
                let spawned = tokio::spawn(async {
                    resolve("session", || async { Err(CoreError::Unauthorized) }).await
                })
                .await
                .unwrap();
                assert!(matches!(spawned, Err(CoreError::Unauthorized)));
            })
            .await;
        assert!(matches!(
            resolve("session", || async { Err(CoreError::Unauthorized) }).await,
            Err(CoreError::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn cancelling_a_request_discards_its_authenticated_identity() {
        let expired = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            CURRENT.scope(RequestActor::new(Some("session".into())), async {
                resolve("session", || async { Ok(user(1)) }).await.unwrap();
                std::future::pending::<()>().await;
            }),
        )
        .await;
        assert!(expired.is_err());
        assert!(matches!(
            resolve("session", || async { Err(CoreError::Unauthorized) }).await,
            Err(CoreError::Unauthorized)
        ));
    }
}
