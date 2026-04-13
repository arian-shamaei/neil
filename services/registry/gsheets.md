# Google Sheets

Read and write Google Sheets via the Google Workspace CLI (gws).

## Account

- **identity**: configured via gws auth (OAuth or service account)
- **scope**: spreadsheets read/write
- **rate limit**: Google API quota (100 requests per 100 seconds per user)

## Actions

### read

Read a range of cells from a spreadsheet.

```
CALL: service=gsheets action=read sheet=<spreadsheet_id> range="Sheet1!A1:D10"
```

| Param | Required | Description |
|-------|----------|-------------|
| sheet | yes | Spreadsheet ID (from the URL) |
| range | yes | A1 notation range (e.g., "Sheet1!A1:D10") |

### append

Append rows to a spreadsheet.

```
CALL: service=gsheets action=append sheet=<id> range="Sheet1!A1" values="Name,Score"
```

| Param  | Required | Description |
|--------|----------|-------------|
| sheet  | yes | Spreadsheet ID |
| range  | yes | Starting cell (e.g., "Sheet1!A1") |
| values | yes | Comma-separated values for one row |

### update

Update a specific cell or range.

```
CALL: service=gsheets action=update sheet=<id> range="Sheet1!B5" values="new value"
```

| Param  | Required | Description |
|--------|----------|-------------|
| sheet  | yes | Spreadsheet ID |
| range  | yes | Cell or range to update |
| values | yes | New value(s) |

### info

Get spreadsheet metadata (title, sheet names, etc.).

```
CALL: service=gsheets action=info sheet=<spreadsheet_id>
```

| Param | Required | Description |
|-------|----------|-------------|
| sheet | yes | Spreadsheet ID |

## Notes

- Spreadsheet ID is the long string in the URL between /d/ and /edit
- Range uses A1 notation: "Sheet1!A1:C10" or just "A1:C10" for first sheet
- Values for multiple columns use comma separation
- All responses are JSON
- Auth must be configured first: `gws auth login` (interactive, one-time)
