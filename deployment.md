# Deployment Configuration

This file is per-installation. Not part of the portable persona.

## Host

- **Machine**: sealserver (Ubuntu VM, Hyper-V)
- **IP**: [server-ip]
- **RAM**: 4GB (dynamic)
- **Disk**: 193GB (4% used)
- **User**: seal
- **NEIL_HOME**: /home/seal/.neil

## Operator

- **Name**: seal
- **Trust level**: full (single user)

## Services

- **systemd**: autoprompt.service (enabled, auto-restart)
- **cron**: heartbeat every 30 minutes
- **Claude**: ~/.local/bin/claude (Anthropic Max plan)

## Network

- **IPv6**: [ipv6]
- **Location**: University of Washington
