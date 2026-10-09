//! [v0.11.3 M6 / addendum-03] Unicode 规范化内核(Unicode **18.0.0**)。
//!
//! **本模块不是一个命名空间** —— 它没有 `SPEC`、不进 `resolve()`、对 wlwl
//! 程序不可见(同 `collection.rs` / `test_native.rs`,已在 `lib.rs` 的
//! `NOT_A_NAMESPACE` 豁免名单里登记)。`wlwl:std.text` 的 `NFC` / `NFD` /
//! `NFC_QC` 三个成员(W-03)把这里的函数转发出去。
//!
//! ## 两半,少一半就静默出错
//!
//! - [`norm`] —— **生成物**(`gen-ucd`,UCD 源数据不入库):四张静态表
//!   (组合类 / 规范分解 / 规范组合 / `NFC_QC` 可疑码点)。
//! - [`canon`] —— 手写算法(UAX #15 的 D68 分解 / D115 规范排序 / D117 规范组合)。
//!
//! ⚠️ **表**不含 11 172 个 Hangul 音节:实测 `UnicodeData.txt` 里 `AC00` 那一行
//! 的分解字段是**空的**(Hangul 音节走 Unicode Standard §3.12 的**算法**分解,
//! 不是表分解)。所以算法那半不能省,表那半也不能省 —— 任何一半单独存在都
//! 会安静地算错。
//!
//! ## 正确性证据在哪
//!
//! **只**在 `norm-check` 对官方 `NormalizationTest.txt` 的**全量**自检里
//! (`addendum-03` §3.1 结论一)。单元测试只抽样钉机制;单测全绿**不能**
//! 当作 `NFC` / `NFD` 正确的证据 —— 规范排序差一个码点时,渲染结果完全
//! 相同,人眼与截图都看不出来。

pub mod canon;
pub mod norm;
