ALTER TABLE sessions ADD INDEX sessions_activity(last_activity);
ALTER TABLE auth_limits ADD INDEX auth_limits_window(window_start);
