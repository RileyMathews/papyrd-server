# Papyrd Server
Papyrd is an eBook server that implements OPDS for eBook discovery and download as well as Kosync for reading progress sync.
Right now it is a minimal implementation that implements basic eBook uploading and the minimal work needed for Kosync and OPDS to work.

# Warning
This project is in alpha. I don't expect things to change much but breaking changes may happen. I will try my absolute best to not have any breaking changes though.

# AI disclosure
As AI is a controversial topic in the self hosting community. I feel the need to disclose that I make regular use of AI tools in my development.
I do not vibe code and review any AI generated code before committing.


# Running

This docker compose example should get the app running but feel free to modify for the specifics of your homelab setup.

```yaml
services:
  papyrd:
    image: ghcr.io/rileymathews/papyrd-server:latest # or tagged release
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgres://papyrd:change-me@postgres:5432/papyrd
      PAPYRD_SESSION_SECRET: change-this-to-a-long-random-string
      PAPYRD_BIND_ADDRESS: 0.0.0.0:3000
      PAPYRD_STORAGE_ROOT: /app/storage # THIS MUST MATCH INGEST STORAGE ROOT IF USING INGEST SERVICE
    volumes:
      - papyrd-storage:/app/storage
    depends_on:
      postgres:
        condition: service_healthy

  papyrd-ingest: # OPTIONAL only use if you want to use the ingest directory
    image: ghcr.io/rileymathews/papyrd-server:latest
    restart: unless-stopped
    command: ["/app/ingest"]
    environment:
      DATABASE_URL: postgres://papyrd:change-me@postgres:5432/papyrd
      PAPYRD_SESSION_SECRET: change-this-to-a-long-random-string
      PAPYRD_STORAGE_ROOT: /app/storage # MAKE SURE THIS MATCHES THE STORAGE ROOT OF YOUR APP SERVICE
      PAPYRD_INGEST_ROOT: /app/ingest
    volumes:
      - papyrd-storage:/app/storage
      - papyrd-ingest:/app/ingest
    depends_on:
      postgres:
        condition: service_healthy

  postgres:
    image: postgres:17-alpine
    restart: unless-stopped
    environment:
      POSTGRES_DB: papyrd
      POSTGRES_USER: papyrd
      POSTGRES_PASSWORD: change-me
    volumes:
      - papyrd-postgres:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U papyrd -d papyrd"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  papyrd-storage:
  papyrd-ingest:
  papyrd-postgres:
```

The ingest container here is completely optional. If running you can drop epub files into the ingest directory via whichever method you have available to you (cp, rsync, scp etc...) and the ingest container will
process the files and add the required metadata to the database before copying the file over to the primary storage directory.

# OPDS
The OPDS entrypoint for your server will be at the `/opds` path. So for example if your server is live at
`https://papyrd.mydomain.com` then you should use `https://papyrd.mydomain.com/opds` in your client configurations.

# Kosync
To use the kosync server for progress syncing you should just configure kosync with the root domain of your server. i.e. `https://papyrd.mydomain.com`
