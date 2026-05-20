use std::collections::HashSet;

use crate::error::AppError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Permission {
    UserPermissionsEdit,
    PublicationUpload,
    PublicationDelete,
}

impl Permission {
    pub const ALL: [Self; 3] = [
        Self::UserPermissionsEdit,
        Self::PublicationUpload,
        Self::PublicationDelete,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserPermissionsEdit => "user.permissions.edit",
            Self::PublicationUpload => "publication.upload",
            Self::PublicationDelete => "publication.delete",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::UserPermissionsEdit => "Edit user permissions",
            Self::PublicationUpload => "Upload books",
            Self::PublicationDelete => "Delete books",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::UserPermissionsEdit => "View users and change their assigned permissions.",
            Self::PublicationUpload => "Upload EPUB files through the web form.",
            Self::PublicationDelete => "Remove books and their stored files from the catalog.",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "user.permissions.edit" => Some(Self::UserPermissionsEdit),
            "publication.upload" => Some(Self::PublicationUpload),
            "publication.delete" => Some(Self::PublicationDelete),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PermissionSet {
    permissions: HashSet<Permission>,
}

impl PermissionSet {
    pub fn new(permissions: impl IntoIterator<Item = Permission>) -> Self {
        Self {
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn contains(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn require(&self, permission: Permission) -> Result<(), AppError> {
        if self.contains(permission) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}
