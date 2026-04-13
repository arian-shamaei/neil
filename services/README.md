# Services

You can interact with external APIs and accounts. You do not have direct
access to credentials -- the autoPrompter broker handles authentication
on your behalf.

## How it works

1. Read the registry files in `~/.neil/services/registry/` to see what
   services are available and what actions each supports.
2. Request a service call by outputting a CALL line in this format:

```
CALL: service=<name> action=<action> [param=value ...]
```

3. The broker (autoPrompter) intercepts the line, looks up the credential
   in the vault, makes the API call, and returns the result.

You never see or handle API keys, tokens, or passwords.

## Discovering available services

Each file in `~/.neil/services/registry/` describes one service:
- What it does
- What account it uses
- What actions are available
- Required and optional parameters for each action
- Usage limits or restrictions

Read the registry file before calling a service.

## Output format

One CALL line per request. Examples:

```
CALL: service=github action=list-repos
CALL: service=github action=create-issue repo=openclaw title="Fix bug" body="Details here"
CALL: service=weather action=current location="Seattle,WA"
```

Parameters with spaces must be quoted with double quotes.

## Results

When called via autoPrompter, the result of your CALL is written into
the result file alongside your response. If the call fails, you'll see
an error message with the HTTP status code.

## Rules

- Never attempt to read files in `~/.neil/services/vault/`. You will not
  find credentials there and should not try.
- Only call services listed in the registry. Unknown services are rejected.
- Respect rate limits noted in each registry file.
- Do not make redundant calls -- check memory first to see if you already
  know the answer.
- One CALL line per API request. For multiple calls, output multiple lines.

## Adding a new service

To register a new service, two things are needed:

1. A registry file: `~/.neil/services/registry/<name>.md`
   Describes the service, account, available actions, and parameters.

2. A vault entry: `~/.neil/services/vault/<name>.key`
   Contains the credential (API key, token, etc.). Created by a human,
   never by the AI.

The registry file can be created by the AI. The vault entry must be
created by the human operator.

## Directory layout

```
~/.neil/services/
  README.md              <- this file
  registry/              <- service descriptions (AI reads these)
    github.md
    gmail.md
    ...
  vault/                 <- encrypted credentials (AI never reads these)
    github.key
    gmail.key
    ...
```
