# pxsol-ss — Complete Setup and Deployment Guide

pxsol-ss is a minimal on-chain data storage program for Solana. Each user has a program-derived PDA, and every invocation replaces the entire contents of that PDA with the instruction data. This repository also includes a single-page web client that uses Phantom.

[English](README.md) | [简体中文](README.zh-CN.md)

## 0. Target configuration

```text
Program ID: 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
Local RPC:  http://127.0.0.1:8899
Devnet RPC: https://api.devnet.solana.com
```

With the repository checked out at `~/pxsol-ss`, the relevant files are:

```text
~/pxsol-ss/
├── Cargo.toml
├── src/
│   └── lib.rs
├── target/
│   └── deploy/
│       ├── pxsol_ss.so
│       └── pxsol_ss-keypair.json
└── client/
    └── index.html
```

`target/` contains build artifacts and is normally ignored by Git. `pxsol_ss-keypair.json` signs for the program address; it is not the program upgrade authority. Keep it private and backed up. It is needed to create the specified Program ID when deploying to a new ledger or cluster. The upgrade authority is set at deployment and is usually the CLI deploy wallet; check the `Authority` field in `solana program show`.

## 1. Toolchain and versions

This project uses the modular Solana SDK and targets SBPFv3. Recommended minimum tool versions:

| Tool | Requirement | Check |
|---|---|---|
| `cargo-build-sbf` | v4.2.0 or later | `cargo build-sbf --version` |
| Solana / Agave CLI | v4.0.0 or later | `solana --version` |
| Rust edition | 2021 | Check `Cargo.toml` |
| Python | Python 3 | `python3 --version` |

Two features in `Cargo.toml` are required:

- `curve25519` on `solana-pubkey`, for `find_program_address`.
- `bincode` on `solana-system-interface`, for constructing `create_account` and `transfer` instructions.

It is normal for modular Solana components to use different version numbers. Do not force them to match manually.

## 2. Build and verify SBPFv3

From the repository root:

```bash
cd ~/pxsol-ss
cargo build-sbf --arch v3 -- -Znext-lockfile-bump
```

The output is `target/deploy/pxsol_ss.so`. Specify `--arch v3` explicitly; otherwise, the resulting bytecode may not be compatible with the target cluster.

Verify the SBPF flags in the ELF header:

```bash
od -An -tx1 -j 0x30 -N 4 target/deploy/pxsol_ss.so
```

Expected output:

```text
03 00 00 00
```

Alternatively:

```bash
LC_ALL=C readelf -h target/deploy/pxsol_ss.so | grep -E 'Machine|Flags'
```

Expect `Machine` to be `Linux BPF` and `Flags` to include `0x3, CPU Version: 3`. `LC_ALL=C` avoids grep failures when `readelf` output is localized.

## 3. Deploy to a local validator

### 3.1 Start the validator

Start the validator in the foreground in terminal A and leave it running:

```bash
cd ~/pxsol-ss
solana-test-validator --reset
```

Wait until its logs show the RPC address `http://127.0.0.1:8899` before continuing. `--reset` clears the local ledger. Omit it if you want to preserve local chain state. Stop a running validator with `Ctrl+C` before resetting its ledger.

### 3.2 Configure the CLI and check the RPC

In terminal B:

```bash
cd ~/pxsol-ss
solana config set --url http://127.0.0.1:8899
solana cluster-version
```

If `cluster-version` returns a version, the CLI is connected to the local validator.

### 3.3 Deploy the program

Check that the build artifact and program keypair exist, and verify the program address:

```bash
ls -l target/deploy/pxsol_ss.so target/deploy/pxsol_ss-keypair.json
solana-keygen pubkey target/deploy/pxsol_ss-keypair.json
```

The keypair-derived address should match the Program ID at the start of this guide. Fund the CLI deploy wallet with local test SOL, then deploy:

```bash
solana airdrop 10
solana program deploy target/deploy/pxsol_ss.so \
  --program-id target/deploy/pxsol_ss-keypair.json
```

Verify the deployment:

```bash
solana program show 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
```

Check that `Owner` is `BPFLoaderUpgradeab1e...` (the Upgradeable Loader) and that `Authority` is the expected deploy wallet. The authority authorizes program upgrades. `Data Length` varies with the build artifact; do not rely on a fixed size.

The local deployment is stored in the validator ledger. After running `solana-test-validator --reset`, deploy the program again.

## 4. Run the web client

Start a static web server in another terminal:

```bash
cd ~/pxsol-ss/client
python3 -m http.server 3000
```

Open <http://localhost:3000> in a browser. The client configuration in [client/index.html](client/index.html) is currently:

```js
const PROGRAM_ID = new PublicKey("4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ");
const RPC = "http://127.0.0.1:8899";
```

In Phantom, select or add a local network pointing to `http://127.0.0.1:8899`, then fund the Phantom wallet address with local test SOL:

```bash
solana airdrop 10 <PHANTOM_WALLET_ADDRESS>
```

Local SOL only exists in the local ledger and cannot be used on Devnet or Mainnet.

The frontend is static HTML; `npm install` is not required. The page loads `@solana/web3.js` from esm.sh, so an internet connection is required to open it.

### Frontend interaction notes

- Browsers do not provide Node.js `Buffer`. Writes use `TextEncoder().encode(text)`. To convert binary data to hex, use `Array.from(u8).map(b => b.toString(16).padStart(2, "0")).join("")`.
- The client signs with `provider.signTransaction(tx)` and broadcasts with `connection.sendRawTransaction(signed.serialize())`. This sends the transaction to the RPC configured by the page instead of relying on Phantom to broadcast through its selected RPC.
- Account order is part of the program ABI and must not be changed:

  ```js
  keys: [
    { pubkey: userKey, isSigner: true,  isWritable: true  }, // [0] user
    { pubkey: pda,     isSigner: false, isWritable: true  }, // [1] PDA
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }, // [2]
    { pubkey: SYSVAR_RENT_PUBKEY,      isSigner: false, isWritable: false }, // [3]
  ]
  ```

- The PDA derivation seed must match the on-chain program:

  ```js
  const [pda] = PublicKey.findProgramAddressSync(
    [userKey.toBuffer()],
    PROGRAM_ID
  );
  ```

## 5. Deploy to Devnet

Devnet and the local validator are separate chains. After deploying to Devnet, the web client's RPC must point to Devnet. The deploy wallet and the Phantom wallet each need their own Devnet SOL.

1. Build and verify SBPFv3 as described in section 2, using `cargo build-sbf --arch v3 -- -Znext-lockfile-bump`.
2. Confirm that `target/deploy/pxsol_ss-keypair.json` is the program-address keypair you control and have backed up, and that its public key matches the client's `PROGRAM_ID`. If the keypair is missing, do not create a replacement and keep using the old Program ID: a new keypair has a different program address, which must also be updated in the client. Also confirm that the CLI wallet is the intended upgrade authority; it usually becomes the authority on initial deployment.
3. Switch the CLI to Devnet, check the connection, and request Devnet SOL for the deploy wallet:

   ```bash
   solana config set --url devnet
   solana cluster-version
   solana airdrop 10
   ```

   If the faucet is rate-limited, use a Solana Devnet faucet to fund the deploy wallet.
4. Deploy and verify:

   ```bash
   solana program deploy target/deploy/pxsol_ss.so \
     --program-id target/deploy/pxsol_ss-keypair.json
   solana program show 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
   ```

5. Confirm that `PROGRAM_ID` in [client/index.html](client/index.html) matches the deployed address, then change the RPC to:

   ```js
   const RPC = "https://api.devnet.solana.com";
   ```

   Restart the web server or refresh the page, switch Phantom to Devnet, and fund the Phantom wallet with Devnet SOL. Writes require transaction fees and the PDA's rent-exempt balance.

On initial deployment, the CLI deploy wallet usually becomes the program upgrade authority. The program-address keypair and the upgrade authority are separate identities: the former determines/signs for the Program ID; the latter authorizes upgrades. Never commit or share private keys.

## 6. Upgrade and authority management

To upgrade the program while keeping the same Program ID, build again and deploy with the CLI wallet that holds the upgrade authority. For an existing program, `--program-id` can be the program address:

```bash
cargo build-sbf --arch v3 -- -Znext-lockfile-bump
solana program deploy target/deploy/pxsol_ss.so \
  --program-id 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
```

Use `solana config get` to check the current CLI signer; it must be the program's current `Authority` or a signer authorized by that authority. When switching between the local validator and Devnet, run `solana config set --url ...` and verify the active cluster before deploying. To transfer upgrade authority, it is safest to specify the new authority's keypair file:

```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --new-upgrade-authority <NEW_AUTHORITY_KEYPAIR>
```

If you provide only the new authority's public key and not its keypair, the CLI may require the explicit `--skip-new-upgrade-authority-signer-check` option. Verify the address carefully before using it.

Revoking upgrade authority is irreversible. Only do this when you are certain the program will never need another upgrade:

```bash
solana program set-upgrade-authority <PROGRAM_ID> --final
```

To clear the local ledger, first stop the validator with `Ctrl+C`, then start it again with `solana-test-validator --reset`.

## 7. Verify write behavior

After connecting a wallet, write the following values in order and observe the session history, PDA data length, and lamports:

| Operation | Program path | Expected result |
|---|---|---|
| First write: `hello solana storage` | Create the PDA and write data | Lamports increase from 0; account data is initialized |
| Write a longer string | Top up rent and expand | Data length increases; lamports usually increase |
| Write `short` | Shrink and refund excess lamports | Data length decreases; lamports usually decrease |
| Write `short` again | Overwrite without changing length | Data length and lamports stay the same |

The rent-exempt balance is recalculated from the current data length. The client displays current on-chain account data; its history timeline is only stored for the current page session and is not an on-chain history.

## 8. Troubleshooting

| Symptom | Common cause | What to do |
|---|---|---|
| `Address already in use` | Another validator is using the local port | Run `solana cluster-version` first. If it returns a version, use the existing validator instead of starting another. |
| `solana cluster-version` hangs | The RPC is not ready, or the port/process is in an inconsistent state | Check validator logs; inspect listening ports with `ss -ltnp \| grep -E ':8899\|:8900'` and identify the relevant process before taking action. |
| `unable to confirm transaction` | Validator is not ready, RPC is unreachable, or deploy wallet has insufficient funds | Wait for the validator, check `solana config get`, and fund the wallet on the currently selected cluster. |
| No output from `readelf`'s `grep` | Output is localized | Use `LC_ALL=C readelf ...` or inspect the header with `od`. |
| Header flags are `0x0` | SBPFv3 was not specified during the build | Rebuild with `cargo build-sbf --arch v3 -- -Znext-lockfile-bump` and check again. |
| `InvalidSeeds` / PDA mismatch | Client program ID or PDA seed does not match the on-chain program | Verify the Program ID and `[userKey.toBuffer()]` seed used by the client. |
| `AccountBorrowFailed` | Conflicting mutable/immutable borrow of the same account | Read and save the required values before taking a mutable borrow. |
| `Buffer is not defined` | Node.js `Buffer` is used in browser code | Use `TextEncoder` and `Uint8Array`. |
| Phantom warns that the network does not match | Phantom's selected network differs from the page RPC | Verify the page RPC is the local address; sign with `signTransaction` and broadcast through the page RPC. If Phantom refuses to sign, switch it to the same local network. |
| `curve25519` compilation error | `solana-pubkey` is missing a feature | Enable `features = ["curve25519"]` in `Cargo.toml`. |
| `create_account` or `transfer` not found | `solana-system-interface` is missing a feature | Enable `features = ["bincode"]` in `Cargo.toml`. |

## 9. Minimal command sequence (fresh local run)

Terminal A:

```bash
cd ~/pxsol-ss
cargo build-sbf --arch v3 -- -Znext-lockfile-bump
od -An -tx1 -j 0x30 -N 4 target/deploy/pxsol_ss.so
solana-test-validator --reset
```

Expect `od` to print `03 00 00 00`. Leave the validator running in terminal A.

Terminal B:

```bash
cd ~/pxsol-ss
solana config set --url http://127.0.0.1:8899
solana cluster-version
solana airdrop 10
solana program deploy target/deploy/pxsol_ss.so \
  --program-id target/deploy/pxsol_ss-keypair.json
solana program show 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
```

Terminal C:

```bash
cd ~/pxsol-ss/client
python3 -m http.server 3000
```

Open <http://localhost:3000>, select the local RPC in Phantom, and connect your wallet to read and write data.
