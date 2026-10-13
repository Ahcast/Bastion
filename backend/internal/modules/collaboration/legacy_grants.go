package collaboration

// mentionGrantClause remains for U2 comment filtering. U3/U4 use the stronger
// grantHolderCanResource path, which also checks the owning module when present.
func mentionGrantClause(permission, owner, team string) (string, []any) {
	return `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id
		WHERE ur.user_id=u.id AND rp.permission=? AND (rp.scope='all' OR (rp.scope='self' AND CAST(u.id AS CHAR)=?) OR
		(rp.scope='team' AND ?<>'' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=? AND tm.user_id=u.id))))`,
		[]any{permission, owner, team, team}
}
