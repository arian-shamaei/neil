# Google Drive

Read, search, and manage files on Google Drive via gws CLI.

## Account

- **identity**: configured via gws auth
- **scope**: drive read/write

## Actions

### list

List files in Drive or a specific folder.

```
CALL: service=gdrive action=list query="name contains 'report'"
```

| Param  | Required | Description |
|--------|----------|-------------|
| query  | no | Drive search query (default: recent files) |
| folder | no | Folder ID to list |

### read

Download a file's content.

```
CALL: service=gdrive action=read file=<file_id>
```

| Param | Required | Description |
|-------|----------|-------------|
| file  | yes | File ID |

### search

Search for files by name or content.

```
CALL: service=gdrive action=search query="quarterly budget"
```

| Param | Required | Description |
|-------|----------|-------------|
| query | yes | Search terms |

## Notes

- File IDs are in the URL or returned by list/search
- Google Docs/Sheets are exported as plain text when read
- For Sheets cell operations, use the gsheets service instead
- Auth shared with gsheets (same gws auth session)
