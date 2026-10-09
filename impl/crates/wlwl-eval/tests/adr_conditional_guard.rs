//! [`ADR-0028`] D1 的**冻结条件守卫** —— 业主 2026-10-09 采纳的那条「绑定」。
//!
//! 采纳的是「A 引入,`HTTP_*` 是阻塞原语」,但 **D1 被冻结在「`ADR-0017`
//! Step 3 仍未落地」这个前提上**:真挂起一旦可用,阻塞的代价**根本不必
//! 承担**,届时 `HTTP_*` 可以直接做挂起形态,连 D2 的 30 s 超时兜底都消失。
//!
//! 本测试把那个前提变成**可观测的判据**。Step 3 落地的那一刻它转红,并
//! 直接说出该做什么 —— 而不是让那句承诺留在文档里等人想起来。
//!
//! ⚠ 为什么值得单独占一个测试:这条绑定是**双向失效**的 —— 提一句「等真挂起
//! 有了就更好」很容易,难的是**在前提已经不成立时仍当它成立**。散文承诺
//! 挡不住这个,只有会红的测试挡得住。
//!
//! [`ADR-0028`]: ../../../docs/adr/0028-std-net-shape.md

use std::path::{Path, PathBuf};

/// `ADR-0017` Step 3「真 state-machine 续跑」的三条落地迹象。
/// 任一成立即视为 Step 3 已落地。
///
/// 判据刻意取**硬证据**(文件存在 / 句子还在),不取行数这类代理指标 ——
/// 代理指标会为了「看起来该变了」而误报,而误报的守卫会被人加豁免,
/// 加了豁免就等于没有守卫。
fn step3_landed_signs() -> Vec<&'static str> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut signs = Vec::new();

    // 迹象一:ADR-0017 §Consequences 自列的新增文件。
    if src.join("effect.rs").is_file() {
        signs.push("src/effect.rs 出现了");
    }

    // 迹象二:`channel.rs` 里那句「只有 `Yield(Explicit)` 保存 env」的说明
    // 消失或改写 —— 那正是 Step 3 要改掉的不变量。
    let channel = src.join("channel.rs");
    let text = std::fs::read_to_string(&channel).expect("src/channel.rs readable");
    if !text.contains("Yield(Explicit)") {
        signs.push("src/channel.rs 里「只有 `Yield(Explicit)` 保存 env」这句不见了");
    }

    signs
}

fn adr_0028() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("adr")
        .join("0028-std-net-shape.md")
}

/// `ADR-0017` Step 3 未落地 ⇒ D1 的冻结前提成立,本测试应当**绿**。
///
/// Step 3 一旦落地 ⇒ 转红,并说明该重新裁决什么。
#[test]
fn adr_0028_d1_freeze_condition_still_holds() {
    let signs = step3_landed_signs();
    assert!(
        signs.is_empty(),
        "ADR-0017 Step 3 已落地({})。\n\
         ADR-0028 的 D1(`HTTP_*` 是阻塞原语)是**在「无可用的真挂起」这个前提下**\n\
         冻结的 —— 前提已经不成立,阻塞的代价就没有必要再承担。请:\n\
          1. 重新裁决 D1(挂起形态 vs 继续阻塞),以及随 D1 而来的 D2 超时兜底;\n\
          2. 更新 ADR-0028 的 Status 与 §4,写明新的裁决与依据;\n\
          3. 若改走挂起,`addendum-05` W-01 的「阻塞原语」那段也要改;\n\
          4. 本测试在裁决落地后应转绿 —— 若新形态仍不具备挂起点,请把下面的判据\n\
             改成**对应新缺口的**证据,不要删掉本测试。",
        signs.join("; ")
    );
}

/// D1 的**记录状态**与本守卫要守的语义对齐 —— ADR 被重写时这条会提醒同步。
#[test]
fn adr_0028_records_the_freeze_condition() {
    let path = adr_0028();
    let text = std::fs::read_to_string(&path).expect("ADR-0028 readable");
    for needle in ["D1", "冻结", "Step 3"] {
        assert!(
            text.contains(needle),
            "ADR-0028 丢了 `{needle}` —— 冻结条件是本 ADR 的一半,不是附注"
        );
    }
    assert!(
        text.contains("**Accepted**"),
        "ADR-0028 的 Status 不再是 Accepted —— 若那是业主撤销裁决,请连同 \
         addendum-05 §7 的 W-01 行一起更新,别只改一处"
    );
}
