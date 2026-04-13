# Deployment Configuration

This file is per-installation. Not part of the portable persona.

## Host

- **Machine**: sealserver (Ubuntu VM, Hyper-V)
- **IP**: 128.95.31.185
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

- **IPv6**: 2607:4000:200:75:215:5dff:fe01:6500
- **Location**: University of Washington
