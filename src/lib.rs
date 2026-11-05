//! # pxsol-ss — Solana 链上极简数据存储器
//!
//! 每个用户对应一个由程序控制的 PDA 账户，用户每次调用本程序，
//! 就把指令携带的字节数据**全量覆盖**写入自己的 PDA。
//!
//! ## 账户模型
//!
//! PDA 地址由 `find_program_address(&[user.key], program_id)` 确定性派生。
//! 同一个用户 + 同一个程序 → 唯一确定的 PDA，无对应私钥，
//! 只有本程序可通过 `invoke_signed` 代表它签名。
//!
//! ## 账户顺序（调用方必须严格遵守）
//!
//! | 索引 | 账户 | 签名 | 可写 | 说明 |
//! |------|------|------|------|------|
//! | [0]  | user | ✅   | ✅   | 签名者、付款方、PDA 派生种子 |
//! | [1]  | pda  | ❌   | ✅   | 用户的专属数据账户（owner = 本程序） |
//! | [2]  | system_program | ❌ | ❌ | CPI 创建账户 / 转账 |
//! | [3]  | rent_sysvar    | ❌ | ❌ | `Rent::get()` 读取租金参数 |
//!
//! **顺序即 ABI**：`next_account_info` 按位置取账户，不做任何名称或类型匹配。
//!
//! ## 指令数据
//!
//! `data: &[u8]` —— 用户要写入 PDA 的完整内容，长度任意。
//! 每次调用都会**全量覆盖**账户中已有的数据，不追加、不合并、不保留历史。
//!
//! ## 执行流程
//!
//! 1. **权限校验**：`user.is_signer` 必须为 `true`。
//! 2. **PDA 校验**：用 `user.key` 重新派生 PDA，要求 `pda.key` 逐字节匹配，
//!    否则返回 `InvalidSeeds`。
//! 3. **计算目标租金**：`rent_exemption = Rent::get()?.minimum_balance(data.len())`。
//!    这是一个**目标绝对值**，只由本次 `data.len()` 决定。
//! 4. **分支 A（未初始化）**：若 `pda.lamports() == 0`，
//!    通过 `invoke_signed` 调用 System Program 的 `create_account` 创建 PDA，
//!    owner 设为 `program_id`，初始 lamports 设为 `rent_exemption`，
//!    随后直接写入 `data` 并提前返回。
//! 5. **分支 B（已存在）**：
//!    - 租金不足 → `invoke` 转账补差；
//!    - 租金超额 → 直接修改 lamports 退回（owner 是自己，无需 CPI）；
//!    - 空间与 `data.len()` 不一致 → `resize` 扩容或缩容；
//!    - `copy_from_slice(data)` 全量覆写。
//!
//! ## 安全边界
//!
//! - `is_signer` 防冒充：证明“这个账户的主人签了名”。
//! - PDA 比对防串空间：证明“这个数据账户就是该用户的专属空间”。
//! - owner = `program_id` 防越权：保证程序有权直接改 PDA 的 `data` 和 `lamports`。
//!
//! 三者缺一不可。只查 `is_signer`，攻击者可用自己的签名写别人的空间；
//! 只查 PDA，攻击者可传任意账户绕过身份绑定。
//!
//! ## 错误码
//!
//! | 错误 | 触发条件 |
//! |------|----------|
//! | `MissingRequiredSignature` | `user` 未签名 |
//! | `InvalidSeeds` | 传入的 PDA 与按 `user.key` 派生的地址不匹配 |
//! | `NotEnoughAccountKeys` | 账户数量少于 4 个（由 `next_account_info` 返回） |
//! | `AccountBorrowFailed` | 同一账户同时 `borrow()` 和 `borrow_mut()` |
//! | `InvalidAccountOwner` | 程序试图修改不属于自己的账户 |
//!
//! ## 设计取舍
//!
//! - **全量覆盖**：语义简单，空间 = 内容 = 租金天然对齐；
//!   代价是不保留历史、不支持部分更新、并发写入会丢更新。
//! - **单种子 PDA**：一个用户在本程序下只有一个存储槽；
//!   若需多种数据分区，应扩展为多种子（如 `[user.key, b"profile"]`）。
//! - **租金每次重算**：不缓存、不累加，按本次 `data.len()` 从头算目标值。
#![allow(unexpected_cfgs)]

use solana_account_info::{next_account_info, AccountInfo};
use solana_cpi::{invoke, invoke_signed};
use solana_msg::msg;
use solana_program_entrypoint::{entrypoint, ProgramResult};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_system_interface::instruction::{create_account, transfer};
use solana_sysvar::Sysvar;

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // ============ 1. 账户反序列化（顺序即 ABI） ============
    let accounts_iter = &mut accounts.iter();
    // 获取AccountMeta
    let account_user = next_account_info(accounts_iter)?;
    let account_data = next_account_info(accounts_iter)?;
    let _system_program = next_account_info(accounts_iter)?;
    let _rent_sysvar = next_account_info(accounts_iter)?;

    msg!("Hello Solana!");

    // ============ 2. 权限校验 ============
    // AccountMeta结构：
    // pub struct AccountMeta {
    //     /// An account's public key.
    //     pub pubkey: Pubkey,
    //     /// True if an `Instruction` requires a `Transaction` signature matching `pubkey`.
    //     pub is_signer: bool,
    //     /// True if the account data or metadata may be mutated during program execution.
    //     pub is_writable: bool,
    // }

    if !account_user.is_signer {
        msg!("User is not a signer");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // ============ 3. PDA 派生与校验 ============
    // 靠 PDA 校验来绑定“account_user ↔ account_data”
    let (pda, bump) = Pubkey::find_program_address(&[&account_user.key.to_bytes()], program_id);

    msg!("PDA: {}, Bump: {}", pda, bump);

    if account_data.key != &pda {
        msg!("data account is not the expected PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    // ============ 4. 目标租金（由 data 长度决定） ============
    // minimum_balance = (账户基础开销 + bytes × 每字节年租金) × 豁免年限
    let rent_exemption = Rent::get()?.minimum_balance(data.len());

    // ============ 5. 分支 A：未初始化 → 创建 PDA ============
    if account_data.lamports() == 0 {
        msg!("PDA not initialized, creating account");

        let ix = create_account(
            account_user.key,  // 付款方
            account_data.key,  // 新账户（PDA）
            rent_exemption,    // 初始 lamports
            data.len() as u64, // 空间大小
            program_id,        // owner = 当前程序
        );

        // 新创建的 PDA 账户本身必须作为签名者，又没有私钥，只能通过invoke_signed "to sign"
        invoke_signed(
            &ix,
            accounts,
            &[&[&account_user.key.to_bytes(), &[bump]]], // PDA 虚拟签名,invoke_signed一次课签多个PDA（所以最外层再套一层）
        )?;

        // owner 是自己，直接写 data，无需 CPI
        account_data.data.borrow_mut().copy_from_slice(data);
        msg!("PDA created and initialized");
        return Ok(());
    }

    // ============ 6. 分支 B：已存在 → 租金调平 ============
    let current_lamports = account_data.lamports();

    if rent_exemption > current_lamports {
        // 差额从用户转给 PDA：付款方是用户（已签名），用普通 invoke
        msg!("topping up rent exemption");
        let ix = transfer(
            account_user.key,
            account_data.key,
            rent_exemption - current_lamports,
        );
        invoke(&ix, accounts)?;
    } else if rent_exemption < current_lamports {
        // 多余租金退回用户：PDA owner 是程序，直接改 lamports，无需 CPI
        msg!("refunding excess rent");
        let excess = current_lamports - rent_exemption;
        let user_lamports = account_user.lamports(); // 先读，避免借用冲突
        **account_user.lamports.borrow_mut() = user_lamports + excess; //Rc<RefCell<&mut u64>>
        **account_data.lamports.borrow_mut() = rent_exemption;
    }

    // ============ 7. 分支 B：扩容 + 覆写 ============
    if account_data.data_len() != data.len() {
        msg!("resizing account data");
        account_data.resize(data.len())?; // v3 下取代已弃用的 realloc
    }
    account_data.data.borrow_mut().copy_from_slice(data);

    msg!("data written successfully");
    Ok(())
}