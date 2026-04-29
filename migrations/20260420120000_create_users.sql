create extension if not exists pgcrypto;

create table users (
    id uuid primary key default gen_random_uuid(),
    username text not null,
    normalized_username text not null,
    password_hash text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint users_normalized_username_key unique (normalized_username)
);
