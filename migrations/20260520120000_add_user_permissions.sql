create type permission as enum (
    'user.permissions.edit',
    'publication.upload',
    'publication.delete'
);

create table user_permissions (
    user_id uuid not null references users (id) on delete cascade,
    permission permission not null,
    granted_at timestamptz not null default now(),
    granted_by_user_id uuid references users (id) on delete set null,
    primary key (user_id, permission)
);

create index user_permissions_permission_idx on user_permissions (permission);
create index user_permissions_granted_by_user_id_idx on user_permissions (granted_by_user_id);

insert into user_permissions (user_id, permission)
select users.id, permissions.permission::permission
from users
cross join (
    values
        ('user.permissions.edit'),
        ('publication.upload'),
        ('publication.delete')
) as permissions(permission)
on conflict do nothing;
