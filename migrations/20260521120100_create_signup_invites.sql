create table signup_invites (
    id uuid primary key default gen_random_uuid(),
    invite_key text not null unique,
    note text,
    created_by_user_id uuid references users (id) on delete set null,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null,
    revoked_at timestamptz,
    used_at timestamptz,
    used_by_user_id uuid references users (id) on delete set null,
    constraint signup_invites_note_not_blank check (note is null or length(btrim(note)) > 0)
);

create index signup_invites_open_idx on signup_invites (expires_at, created_at)
where revoked_at is null and used_at is null;

create index signup_invites_created_by_user_id_idx on signup_invites (created_by_user_id);
create index signup_invites_used_by_user_id_idx on signup_invites (used_by_user_id);

insert into user_permissions (user_id, permission, granted_by_user_id)
select user_id, 'user.invites.create'::permission, granted_by_user_id
from user_permissions
where permission = 'user.permissions.edit'
on conflict do nothing;
