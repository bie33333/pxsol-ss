# pxsol-ss 完整运行与部署指南

[English](README.md) | 简体中文

pxsol-ss 是一个 Solana 链上极简数据存储程序。每个用户对应一个由程序派生的 PDA；用户每次调用程序时，都会用指令数据完整覆盖该 PDA 中的内容。仓库还包含一个使用 Phantom 的单文件网页客户端。

## 0. 目标配置

```text
Program ID: 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
本地 RPC:   http://127.0.0.1:8899
Devnet RPC: https://api.devnet.solana.com
```

仓库根目录为 `~/pxsol-ss` 时，主要文件如下：

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

`target/` 是构建产物目录，通常被 Git 忽略。`pxsol_ss-keypair.json` 是用于签署程序地址的 keypair，不是程序升级 authority；不要公开或提交，建议备份。初次部署到新 ledger/集群时需要它来创建指定 Program ID。程序的升级权限由部署时设置的 authority 持有，通常是 CLI 部署钱包；以 `solana program show` 显示的 `Authority` 为准。

## 1. 环境与版本

本项目使用 Solana 组件化 SDK，构建目标为 SBPFv3。建议使用以下工具版本：

| 工具 | 要求 | 检查命令 |
|---|---|---|
| `cargo-build-sbf` | v4.2.0 或更高 | `cargo build-sbf --version` |
| Solana / Agave CLI | v4.0.0 或更高 | `solana --version` |
| Rust edition | 2021 | 查看 `Cargo.toml` |
| Python | Python 3 | `python3 --version` |

`Cargo.toml` 中的两个 feature 不能漏：

- `solana-pubkey` 的 `curve25519`：用于 `find_program_address`。
- `solana-system-interface` 的 `bincode`：用于构造 `create_account` 和 `transfer` 指令。

组件依赖版本号不完全相同是正常的，不要为了“统一版本号”手动强行对齐。

## 2. 编译并验证 SBPFv3

在仓库根目录执行：

```bash
cd ~/pxsol-ss
cargo build-sbf --arch v3 -- -Znext-lockfile-bump
```

产物为 `target/deploy/pxsol_ss.so`。必须显式指定 `--arch v3`，否则可能构建出与目标集群不兼容的字节码。

验证 ELF header 中的 SBPF flags：

```bash
od -An -tx1 -j 0x30 -N 4 target/deploy/pxsol_ss.so
```

期望输出：

```text
03 00 00 00
```

也可以使用：

```bash
LC_ALL=C readelf -h target/deploy/pxsol_ss.so | grep -E 'Machine|Flags'
```

期望 `Machine` 为 `Linux BPF`，`Flags` 包含 `0x3, CPU Version: 3`。`LC_ALL=C` 可避免系统语言设置导致 `grep` 匹配不到本地化输出。

## 3. 本地验证器部署

### 3.1 启动验证器

在终端 A 前台启动验证器，保持该终端运行：

```bash
cd ~/pxsol-ss
solana-test-validator --reset
```

等待日志显示 RPC 地址 `http://127.0.0.1:8899` 后，再进行下一步。`--reset` 会清空本地 ledger；如果要保留本地链状态，不要使用此参数。清空账本前先用 `Ctrl+C` 停止正在运行的验证器。

### 3.2 配置 CLI 并检查 RPC

在终端 B 执行：

```bash
cd ~/pxsol-ss
solana config set --url http://127.0.0.1:8899
solana cluster-version
```

`cluster-version` 能返回版本号即表示 CLI 已连上本地验证器。

### 3.3 部署程序

确认构建产物和程序 keypair 都存在，并核对程序地址：

```bash
ls -l target/deploy/pxsol_ss.so target/deploy/pxsol_ss-keypair.json
solana-keygen pubkey target/deploy/pxsol_ss-keypair.json
```

keypair 派生出的地址应为本指南开头的 Program ID。部署钱包需要有本地测试 SOL，然后部署：

```bash
solana airdrop 10
solana program deploy target/deploy/pxsol_ss.so \
  --program-id target/deploy/pxsol_ss-keypair.json
```

验证部署：

```bash
solana program show 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
```

确认输出中的 `Owner` 是 `BPFLoaderUpgradeab1e...`（Upgradeable Loader），并检查 `Authority` 是预期的部署钱包地址。这个 authority 才能授权升级；`Data Length` 会随构建产物而变化，不要依赖固定字节数。

本地部署的程序状态保存在 validator ledger 中。执行 `solana-test-validator --reset` 后，需要重新部署。

## 4. 启动网页客户端

在另一个终端启动静态网页服务器：

```bash
cd ~/pxsol-ss/client
python3 -m http.server 3000
```

浏览器打开 <http://localhost:3000>。客户端目前在 [client/index.html](client/index.html) 中配置为：

```js
const PROGRAM_ID = new PublicKey("4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ");
const RPC = "http://127.0.0.1:8899";
```

在 Phantom 中选择或添加指向 `http://127.0.0.1:8899` 的本地网络，并为该钱包地址领取本地测试 SOL：

```bash
solana airdrop 10 <PHANTOM_WALLET_ADDRESS>
```

本地 SOL 只存在于本地 ledger，与 Devnet 或 Mainnet SOL 不通用。

前端是静态 HTML，不需要执行 `npm install`。页面通过 esm.sh 加载 `@solana/web3.js`，因此打开页面需要网络连接。

### 前端交互要点

- 浏览器没有 Node.js 的 `Buffer`。写入使用 `TextEncoder().encode(text)`；二进制数据转十六进制可使用 `Array.from(u8).map(b => b.toString(16).padStart(2, "0")).join("")`。
- 客户端使用 `provider.signTransaction(tx)` 获取签名，再调用 `connection.sendRawTransaction(signed.serialize())` 广播。这样交易发往页面配置的 RPC，而不是依赖 Phantom 选择的 RPC 代为广播。
- 指令账户顺序是程序 ABI，不能调整：

  ```js
  keys: [
    { pubkey: userKey, isSigner: true,  isWritable: true  }, // [0] user
    { pubkey: pda,     isSigner: false, isWritable: true  }, // [1] PDA
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }, // [2]
    { pubkey: SYSVAR_RENT_PUBKEY,      isSigner: false, isWritable: false }, // [3]
  ]
  ```

- PDA 派生种子必须与链上程序一致：

  ```js
  const [pda] = PublicKey.findProgramAddressSync(
    [userKey.toBuffer()],
    PROGRAM_ID
  );
  ```

## 5. 部署到 Devnet

Devnet 和本地验证器是两条独立的链。部署到 Devnet 后，网页客户端的 RPC 必须改为 Devnet；部署账户和 Phantom 钱包也需要各自有 Devnet SOL。

1. 按第 2 节使用 `cargo build-sbf --arch v3 -- -Znext-lockfile-bump` 构建并验证 SBPFv3。
2. 确认 `target/deploy/pxsol_ss-keypair.json` 是你控制并备份好的程序地址 keypair，且其公钥与客户端 `PROGRAM_ID` 相同。若 keypair 不存在，不要随意新建后仍沿用旧 Program ID；新 keypair 会对应新的程序地址，必须同步更新客户端。另确认 CLI 当前配置的钱包是你预期的升级 authority；首次部署时该钱包通常会成为 authority。
3. 把 CLI 切换到 Devnet，检查连接，并为部署钱包领取 Devnet SOL：

   ```bash
   solana config set --url devnet
   solana cluster-version
   solana airdrop 10
   ```

   水龙头有额度限制时，使用 Solana Devnet faucet 为部署钱包充值。
4. 部署并验证：

   ```bash
   solana program deploy target/deploy/pxsol_ss.so \
     --program-id target/deploy/pxsol_ss-keypair.json
   solana program show 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
   ```

5. 确认 [client/index.html](client/index.html) 中的 `PROGRAM_ID` 与部署地址一致，并将 RPC 改为：

   ```js
   const RPC = "https://api.devnet.solana.com";
   ```

   重启网页服务器或刷新页面，在 Phantom 中切换到 Devnet，并为 Phantom 钱包领取 Devnet SOL。写入 PDA 需要支付交易手续费和 PDA 免租金余额。

首次部署时，CLI 部署钱包通常会成为程序的升级 authority。程序地址 keypair 和升级 authority 是两个不同的身份：前者确定/签署 Program ID，后者授权升级。不要把任何私钥提交到仓库或发给他人。

## 6. 升级与权限管理

程序升级时保持 Program ID 不变，使用持有升级 authority 的 CLI 钱包重新构建并部署。部署到已有程序时，`--program-id` 可以直接使用程序地址：

```bash
cargo build-sbf --arch v3 -- -Znext-lockfile-bump
solana program deploy target/deploy/pxsol_ss.so \
  --program-id 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
```

`solana config get` 可检查当前 CLI signer；它必须是程序当前 `Authority`，或由相应 authority 授权的 signer。切换本地验证器和 Devnet 时，先用 `solana config set --url ...` 确认当前集群，避免把程序部署到错误的网络。转移升级权限时，最好指定新 authority 的 keypair 文件：

```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --new-upgrade-authority <NEW_AUTHORITY_KEYPAIR>
```

如果只提供新 authority 的公钥而没有对应 keypair，CLI 可能要求显式添加 `--skip-new-upgrade-authority-signer-check`；使用前务必核对地址。

永久撤销升级权限不可逆，执行前务必确认不再需要升级：

```bash
solana program set-upgrade-authority <PROGRAM_ID> --final
```

清空本地账本时，先在 validator 终端按 `Ctrl+C` 停止验证器，再重新运行 `solana-test-validator --reset`。

## 7. 验证写入行为

连接钱包后，依次写入以下内容，并观察网页中的本次会话历史、PDA 字节数和 lamports：

| 操作 | 程序路径 | 预期现象 |
|---|---|---|
| 首次写入 `hello solana storage` | 创建 PDA 并写入 | lamports 从 0 增加，账户数据初始化 |
| 写入更长文本 | 补租金并扩容 | 数据长度增加，通常 lamports 增加 |
| 写入 `short` | 缩容并退回多余 lamports | 数据长度减少，通常 lamports 减少 |
| 再写入 `short` | 等长覆写 | 数据长度和 lamports 不变 |

租金豁免余额按本次数据长度重新计算。客户端显示的是链上当前账户数据；历史时间线仅保存在当前页面会话中，不是链上历史记录。

## 8. 常见问题

| 症状 | 常见原因 | 处理方式 |
|---|---|---|
| `Address already in use` | 已有 validator 占用本地端口 | 先运行 `solana cluster-version`；若能返回版本，直接使用现有 validator，不要再启动一个 |
| `solana cluster-version` 无响应 | RPC 没有就绪，或端口/进程状态异常 | 查看 validator 终端日志；用 `ss -ltnp \| grep -E ':8899|:8900'` 检查监听端口，确认后再处理对应进程 |
| `unable to confirm transaction` | validator 尚未就绪、RPC 未连通或部署钱包余额不足 | 等待 validator 就绪，检查 `solana config get`，再为当前集群的钱包充值 |
| `readelf` 的 `grep` 没有输出 | locale 输出不是英文 | 使用 `LC_ALL=C readelf ...`，或使用 `od` 检查 header |
| header flags 是 `0x0` | 构建时没有指定 SBPFv3 | 重新运行 `cargo build-sbf --arch v3 -- -Znext-lockfile-bump` 并复查 |
| `InvalidSeeds` / PDA 不匹配 | 前端的程序 ID 或 PDA 种子与链上不一致 | 确认前端使用正确 Program ID 和 `[userKey.toBuffer()]` 种子 |
| `AccountBorrowFailed` | 同一账户存在冲突的可变/不可变数据借用 | 先读取并保存需要的值，再进行可变借用 |
| `Buffer is not defined` | 在浏览器代码中使用 Node.js `Buffer` | 使用 `TextEncoder` 和 `Uint8Array` |
| Phantom 与本地网络不一致提示 | Phantom 当前选择的网络和页面 RPC 不同 | 确认页面 RPC 是本地地址；用 `signTransaction` 签名并由页面 RPC 广播，若 Phantom 不允许签名则切换到相同的本地网络 |
| `curve25519` 相关编译错误 | `solana-pubkey` 缺少 feature | 在 `Cargo.toml` 启用 `features = ["curve25519"]` |
| `create_account` 或 `transfer` 找不到 | `solana-system-interface` 缺少 feature | 在 `Cargo.toml` 启用 `features = ["bincode"]` |

## 9. 最小命令序列（本地从零运行）

终端 A：

```bash
cd ~/pxsol-ss
cargo build-sbf --arch v3 -- -Znext-lockfile-bump
od -An -tx1 -j 0x30 -N 4 target/deploy/pxsol_ss.so
solana-test-validator --reset
```

期望 `od` 输出 `03 00 00 00`。验证器启动后保持终端 A 运行。

终端 B：

```bash
cd ~/pxsol-ss
solana config set --url http://127.0.0.1:8899
solana cluster-version
solana airdrop 10
solana program deploy target/deploy/pxsol_ss.so \
  --program-id target/deploy/pxsol_ss-keypair.json
solana program show 4C4tdiGW8HK9jnKQDEDtqh6xuKdtPpzE5yFNREi4tNiZ
```

终端 C：

```bash
cd ~/pxsol-ss/client
python3 -m http.server 3000
```

打开 <http://localhost:3000>，在 Phantom 中选择本地 RPC 并连接钱包，即可读取和写入。
