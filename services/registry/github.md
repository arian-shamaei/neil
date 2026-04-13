# GitHub API

External service for interacting with GitHub repositories, issues, and pull requests.

## Account

- **identity**: seal (github.com/seal)
- **scope**: read/write on owned repos
- **rate limit**: 5000 requests/hour

## Actions

### list-repos

List repositories for the authenticated user.

```
CALL: service=github action=list-repos
```

No parameters required.

### get-repo

Get details about a specific repository.

```
CALL: service=github action=get-repo repo=<owner/repo>
```

| Param | Required | Description |
|-------|----------|-------------|
| repo  | yes      | owner/repo format |

### list-issues

List open issues for a repository.

```
CALL: service=github action=list-issues repo=<owner/repo>
```

| Param  | Required | Description |
|--------|----------|-------------|
| repo   | yes      | owner/repo format |
| state  | no       | open, closed, all (default: open) |
| labels | no       | comma-separated label filter |

### create-issue

Create a new issue.

```
CALL: service=github action=create-issue repo=<owner/repo> title="Title" body="Body"
```

| Param | Required | Description |
|-------|----------|-------------|
| repo  | yes      | owner/repo format |
| title | yes      | issue title |
| body  | no       | issue body (markdown) |

### get-pull

Get details about a pull request.

```
CALL: service=github action=get-pull repo=<owner/repo> number=<n>
```

| Param  | Required | Description |
|--------|----------|-------------|
| repo   | yes      | owner/repo format |
| number | yes      | PR number |

## Notes

- All responses are JSON.
- Respect the rate limit. Check memory before making redundant calls.
- The vault credential is a GitHub Personal Access Token (classic).
