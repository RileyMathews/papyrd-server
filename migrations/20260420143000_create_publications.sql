create table publications (
    id uuid primary key,
    source_identifier text not null,
    title text not null,
    sort_title text,
    description text,
    language_code text,
    publisher text,
    published_on date,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint publications_source_identifier_key unique (source_identifier)
);

create index publications_sort_title_idx on publications (sort_title);
create index publications_created_at_idx on publications (created_at);
create index publications_updated_at_idx on publications (updated_at);

create table contributors (
    id uuid primary key,
    name text not null,
    sort_name text,
    created_at timestamptz not null default now()
);

create table publication_contributors (
    publication_id uuid not null references publications (id) on delete cascade,
    contributor_id uuid not null references contributors (id) on delete cascade,
    role text not null,
    position integer,
    primary key (publication_id, contributor_id, role)
);

create index publication_contributors_publication_id_idx on publication_contributors (publication_id);
create index publication_contributors_contributor_id_idx on publication_contributors (contributor_id);

create table assets (
    id uuid primary key,
    publication_id uuid not null references publications (id) on delete cascade,
    kind text not null,
    storage_path text not null,
    media_type text not null,
    byte_size bigint,
    partial_md5 text,
    width integer,
    height integer,
    original_filename text,
    created_at timestamptz not null default now()
);

create index assets_publication_id_idx on assets (publication_id);
create index assets_kind_idx on assets (kind);
create index assets_partial_md5_idx on assets (partial_md5);
