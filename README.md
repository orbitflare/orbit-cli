<p align="center">
  <img src="https://raw.githubusercontent.com/orbitflare/orbit-cli/main/assets/banner.png" alt="OrbitFlare CLI" width="100%"/>
</p>

Real-time streaming (Jetstream, Yellowstone gRPC, WebSocket), RPC and DAS queries, transaction landing through Apex, project scaffolding and account management.

## Install

```bash
cargo install orbitflare
```

Or from source:

```bash
git clone https://github.com/orbitflare/orbit-cli.git
cd orbit-cli
cargo install --path .
```

Requires Rust 1.92+.

## Quick Start

```bash
# Authenticate
orbitflare auth login --x-orbit-key <YOUR_KEY>

# Set your RPC license key
orbitflare auth set-license-key <LICENSE_KEY>

# Pick the region closest to you (default: fra)
orbitflare config set region ams

# Check connectivity
orbitflare ping

# Query the blockchain
orbitflare rpc slot
orbitflare rpc balance <ADDRESS>
orbitflare das owner <WALLET>

# Stream via Jetstream
orbitflare jet --required 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P --enrichment
orbitflare jet --config stream.yml

# Stream via Yellowstone gRPC
orbitflare grpc --config grpc-stream.yml

# Scaffold a project from a template
orbitflare template --list
orbitflare template --install solana-copy-trader

# Launch the TUI dashboard
orbitflare dashboard
```

## Authentication

Three login methods:

```bash
# API key (from orbitflare.com/dashboard)
orbitflare auth login --x-orbit-key <KEY>

# Solana wallet keypair
orbitflare auth login --wallet ~/.config/solana/id.json

# Device flow (browser-based)
orbitflare auth login
```

The API key (`X-ORBIT-KEY`) authenticates against the OrbitFlare Customer API. The license key authenticates RPC endpoints and is fetched automatically during login, or set manually. gRPC and Jetstream use IP whitelisting.

```bash
orbitflare auth set-license-key <KEY>
```

Apex sends authenticate with an Apex API key, stored per profile or read from `ORBITFLARE_APEX_KEY`:

```bash
orbitflare auth set-apex-key <KEY>
```

Credentials are stored in the OS keychain. Check status with:

```bash
orbitflare auth status
```

## Commands

### Core (always available)

| Command | Description |
|---------|-------------|
| `auth login` | Authenticate (API key, wallet, or device flow) |
| `auth logout` | Remove stored credentials |
| `auth status` | Show authentication status |
| `auth switch --profile <NAME>` | Switch auth profile |
| `auth set-license-key <KEY>` | Set RPC license key |
| `auth set-apex-key <KEY>` | Set Apex API key |
| `config show` | Show configuration |
| `config set <KEY> <VALUE>` | Set a config value |
| `config remove <KEY> <VALUE>` | Remove a value from a list config |
| `config reset` | Reset to defaults |
| `ping` | Check service connectivity |
| `ping --service <SVC>` | Ping a specific service (rpc, jetstream, grpc) |
| `apex ping` | Measure latency to each Apex region |
| `docs [TOPIC...]` | Open docs in browser (`cli`, `rpc`, `ws`, `jet`, `grpc`, `das`, `history`, `apex`, `sdk`, `api`, or a docs path) |
| `plan list` | List available RPC plans |
| `plan view <SLUG>` | View plan details |
| `plan compare --all` | Compare all plans side by side |
| `pay check-balance` | Check account balance |
| `pay topup <AMOUNT>` | Top up balance with USDC (on-chain) |
| `pay history` | Top-up / credit history |
| `pay purchase <SLUG> <PERIOD>` | Purchase a plan from balance |
| `pay order --history` | List orders |
| `pay invoice --list` | List invoices |
| `pay invoice <REF>` | View invoice details |
| `pay renew <INVOICE>` | Pay invoice from balance |

### Streaming & Queries

**jet** -- Real-time Jetstream transaction streaming

```bash
orbitflare jet --config stream.yml
orbitflare jet --required <PROGRAM_ID> --enrichment
orbitflare jet --include <ADDR> --exclude <ADDR> --duration 30
orbitflare jet --slots
orbitflare jet --config stream.yml --v1
```

`jet` speaks Jetstream v2 by default. `--enrichment` (or `include_enrichment: true` on any filter) adds the fee payer, compute-unit price and limit, priority fee, program IDs, writable accounts, size and resolved lookup-table addresses to every transaction. `--slots` streams slot lifecycle events (alive, complete, dead). `--v1` keeps the deprecated v1 protocol. Jetstream streams transactions only: filters take `account_include`, `account_exclude` and `account_required`; for account updates use `grpc`. `--json` prints one JSON object per event.

**grpc** -- Real-time Yellowstone gRPC (Geyser) streaming

```bash
orbitflare grpc --config grpc-stream.yml
```

Yellowstone adds commitment level control, vote/failed tx filtering, slot updates, block metadata, and transaction fees over Jetstream. See `examples/jet-stream.yml` and `examples/grpc-stream.yml` for full config references.

**ws** -- Solana WebSocket subscriptions

```bash
orbitflare ws slot
orbitflare ws account <ADDRESS>
orbitflare ws logs --mentions <ADDRESS>
orbitflare ws signature <SIGNATURE>
```

The WebSocket endpoint is derived from the RPC endpoint (`https://` becomes `wss://`); override it with `--ws-url`.

**rpc** -- Solana RPC queries

```bash
orbitflare rpc account <ADDRESS>
orbitflare rpc tokens <WALLET>
orbitflare rpc balance <ADDRESS>
orbitflare rpc tx <SIGNATURE>
orbitflare rpc history <ADDRESS> --limit 20
orbitflare rpc history <ADDRESS> --oldest-first --status failed --since 2026-01-01 --token-accounts balanceChanged
orbitflare rpc history <ADDRESS> --page-token <TOKEN>
orbitflare rpc slot
orbitflare rpc epoch
orbitflare rpc blockhash
orbitflare rpc stats
orbitflare rpc priority-fees --account <ADDR>
orbitflare rpc raw '{"jsonrpc":"2.0","id":1,"method":"getSlot","params":[]}'
```

`rpc history` uses `getTransactionsForAddress`: newest or oldest first, status and time filters, token-account activity and pagination. `rpc tokens` lists Token and Token-2022 accounts.

**das** -- Digital Asset Standard API (tokens, NFTs, compressed NFTs)

```bash
orbitflare das asset <ID> [<ID>...]
orbitflare das owner <WALLET>                 # SOL, priced tokens and NFTs
orbitflare das owner <WALLET> --tokens        # or --nfts
orbitflare das search --owner <WALLET> --token-type compressedNFT
orbitflare das search --creator <ADDR> --burnt false
orbitflare das collection <COLLECTION>
orbitflare das holders <MINT>
orbitflare das proof <ASSET_ID>
orbitflare das signatures <ASSET_ID>
```

List commands take `--limit` (up to 1000) and `--page`.

**apex** -- Send transactions through Apex

```bash
orbitflare apex ping
orbitflare apex tip-accounts --region fra
orbitflare apex send <BASE64_TX> --region fra --confirm
orbitflare apex send --file tx.b64
orbitflare apex transfer <TO> <SOL> --wallet /path/to/keypair.json --tip 0.001
```

Regions: `global`, `fra`, `ams`, `dub`, `lon`, `nyc`, `slc`, `sgp`, `tyo`, `sqq`. `ping` and `tip-accounts` need no key. Every transaction must carry exactly one top-level SystemProgram transfer of at least 0.001 SOL to an Apex tip account, in its static account keys; `send` checks this before sending and `transfer` adds it for you. Confirmation uses the configured RPC endpoint.

**template** -- Project scaffolding

```bash
orbitflare template --list
orbitflare template --list --filter rust
orbitflare template --view solana-copy-trader
orbitflare template --install solana-copy-trader --dir ./my-bot
```

Templates are fetched from the configured repo (default: `github.com/orbitflare/templates`).

**plan** -- Browse and compare RPC plans

```bash
orbitflare plan list
orbitflare plan view <SLUG>
orbitflare plan compare --all
orbitflare plan compare dev growth
```

Plan data is cached locally for 6 hours. Without a login the public catalog is shown.

**pay** -- Payments, top-ups, and plan purchases

```bash
# Balance
orbitflare pay check-balance

# Top up with USDC (on-chain)
orbitflare pay topup <AMOUNT>
orbitflare pay topup <AMOUNT> --wallet /path/to/keypair.json
orbitflare pay history

# Purchase a plan (from balance)
orbitflare pay purchase <SLUG> <PERIOD> --coupon <CODE>
# period: monthly | quarterly | semi-annual | annual

# Orders
orbitflare pay order --history
orbitflare pay order <REFERENCE>

# Invoices
orbitflare pay invoice --list
orbitflare pay invoice <REFERENCE>

# Pay invoice from balance (renewal)
orbitflare pay renew <INVOICE_REF>
```

Only `topup` requires a wallet keypair (defaults to `~/.config/solana/id.json`). All other pay commands use your account balance.

**dashboard** -- TUI dashboard for account management

```bash
orbitflare dashboard
```

Sections: Home, Services, Invoices, API Keys, Profile. Full keyboard navigation. With a license key the header shows live token prices from the DAS API.

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Esc` | Back |
| `Tab` | Switch tab / focus |
| `↑↓` / `jk` | Navigate |
| `Enter` | Select / expand / toggle |
| `t` / `T` | Cycle theme |
| `[` | Toggle sidebar |
| `R` | Refresh data |

Service detail keybinds:

| Key | Tab | Action |
|-----|-----|--------|
| `m` | Details | Toggle auth mode (IP Whitelist / API Key) |
| `w` | Details | Toggle auto-renewal |
| `r` | Details | Regenerate license key |
| `n` | IPs / Domains | Switch Mainnet / Devnet |
| `a` | IPs / Domains | Add IP or domain |
| `d` | IPs / Domains | Remove IP or domain |

API Keys keybinds:

| Key | Action |
|-----|--------|
| `c` | Create new API key |
| `e` | Toggle enabled/disabled |
| `r` | Regenerate key |
| `x` | Delete key |
| `a` | Add IP (when expanded) |
| `d` | Remove IP (when expanded) |

Invoices keybinds:

| Key | Action |
|-----|--------|
| `p` | Pay with balance (pending invoices) |

Profile keybinds:

| Key | Action |
|-----|--------|
| `e` | Edit name and payment wallet |

## Configuration

Config file: `~/.orbitflare/config.yml`

```bash
orbitflare config show
orbitflare config set region ams
orbitflare config set network devnet
orbitflare config set rpc.url https://your-rpc-endpoint
orbitflare config set grpc.url http://your-grpc-endpoint:10000
orbitflare config set jetstream.url http://your-jetstream-endpoint
orbitflare config set templates.repo https://github.com/your-org/templates
```

Endpoints default to the configured region (`fra` when unset); an explicit `rpc.url`, `grpc.url` or `jetstream.url` takes precedence.

| Service | Endpoint | Regions |
|---------|----------|---------|
| RPC / WebSocket | `https://{region}.rpc.orbitflare.com` | `va`, `ny`, `la`, `slc`, `ams`, `fra`, `lon`, `jp`, `sg` |
| Yellowstone gRPC | `http://{region}.rpc.orbitflare.com:10000` | same as RPC |
| Jetstream | `http://{region}.jetstream.orbitflare.com` | `ny`, `slc`, `ams`, `fra`, `lon`, `dub`, `sqq`, `jp`, `sgp` |

RPC regions without a Jetstream endpoint map to the nearest one (`va` → `ny`, `la` → `slc`, `sg` → `sgp`).

Devnet endpoints are preconfigured:
- RPC: `https://devnet.rpc.orbitflare.com` (fallback: `https://api.devnet.solana.com`)
- gRPC: `http://devnet.rpc.orbitflare.com:10000`

Use `--network devnet` to switch at runtime. Jetstream is mainnet only.

## Global Flags

```
--json              Output raw JSON
--raw               Minimal output
--network <NET>     Override network (mainnet|devnet)
--region <CODE>     Region for default endpoints
--commitment <C>    Override commitment (processed|confirmed|finalized)
--rpc-url <URL>     Override RPC endpoint
--grpc-url <URL>    Override Yellowstone gRPC endpoint
--jetstream-url <U> Override Jetstream endpoint
--ws-url <URL>      Override WebSocket endpoint (default: derived from RPC URL)
--fallback-url <U>  Fallback URLs (repeatable)
--profile <NAME>    Use a specific auth profile
--quiet             Suppress non-essential output
--no-color          Disable colors
```
