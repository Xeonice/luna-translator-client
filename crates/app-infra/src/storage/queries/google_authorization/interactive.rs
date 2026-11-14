//! Metadata-only settings reads on the real writer. No latest-grant discovery.
use super::*;
use app_core::ports::google_interactive::GoogleInteractiveStoredState;
impl SqliteStore {
    pub fn google_interactive_state(
        &self,
        generation: NonZeroU32,
        reference: Option<GoogleAuthorizationRef>,
    ) -> AppResult<GoogleInteractiveStoredState> {
        self.call(move|db| {
            let actual=bound_runtime_frame(db)?;
            if actual.profile_generation!=generation {return Err(failure("google_profile_stale"));}
            let context=handler_context(db);
            if !handler_ready(&db.conn,&context)? {return Err(failure("google_handler_not_installed"));}
            let handler=context.registration.ok_or_else(||failure("google_handler_not_installed"))?;
            let (grant,count)=if let Some(reference)=reference {
                require_id(&reference.grant_id)?;
                let grant=authority::grant(&db.conn,&reference.grant_id)?;
                if grant.state==GoogleGrantState::Active {
                    let material=grant.material.as_ref().ok_or_else(||failure("google_material_not_found"))?;
                    if authority::active_material(&db.conn,&material.credential.credential_id,CredentialMaterialKind::GoogleOauthRefresh)?!=*material {return Err(failure("google_material_changed"));}
                }
                let count:u32=db.conn.query_row("SELECT count(DISTINCT c.connection_id) FROM connections c JOIN config_heads h ON h.revision_id=c.revision_id WHERE c.auth_mode='google_oauth' AND c.oauth_grant_id=?1",[&reference.grant_id],|r|r.get(0)).map_err(err)?;
                (Some(grant),count)
            } else {(None,0)};
            Ok(GoogleInteractiveStoredState{frame:actual,handler,grant,affected_service_count:count})
        })
    }
}
