alter table users
    add column kosync_userkey_hash text;

create table reading_progress (
    user_id uuid not null references users (id) on delete cascade,
    document text not null,
    progress text not null,
    percentage double precision not null,
    device text not null,
    device_id text,
    updated_at timestamptz not null default now(),
    primary key (user_id, document)
);

create index reading_progress_updated_at_idx on reading_progress (updated_at);
