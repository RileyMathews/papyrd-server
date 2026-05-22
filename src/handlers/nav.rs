use crate::permissions::{Permission, PermissionSet};

#[derive(Clone, Copy, Debug)]
pub struct NavView {
    pub can_edit_permissions: bool,
    pub can_invite_users: bool,
    pub can_upload_publications: bool,
    pub can_delete_publications: bool,
}

impl NavView {
    pub fn from_permissions(permissions: &PermissionSet) -> Self {
        Self {
            can_edit_permissions: permissions.contains(Permission::UserPermissionsEdit),
            can_invite_users: permissions.contains(Permission::UserInvitesCreate),
            can_upload_publications: permissions.contains(Permission::PublicationUpload),
            can_delete_publications: permissions.contains(Permission::PublicationDelete),
        }
    }
}
