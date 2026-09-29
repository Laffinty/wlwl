//! `cargo test -p wlwl-cli --test conformance_static`
//!
//! 计划书 §12.1 点名的验收命令。它跑的是**真的二进制**(`wlwl check`),
//! 不是内部函数 —— 于是它同时验到三件事:
//!
//! 1. **S2 开启可抓错**:门开着时,每份夹具的期望码**必须**出现;
//! 2. **S1 默认零破坏**:同一批夹具在默认档(无清单)下**一条静态诊断都没有**
//!    且 `wlwl check` 通过(所以夹具一定**解析干净** —— 静态诊断不可能是
//!    语法错冒充的);
//! 3. `mode: both` 的控制组还必须 `wlwl run` 跑通,证明这套夹具与这套
//!    断言都不是摆设。故意违例的夹具**不**要求能跑:静态类型违例在运行期
//!    多半**也**是 `E0030`,那是 E0030 存在的意义。
//!
//! 夹具的期望写在**夹具文件的注释里**(见 `impl/tests/conformance/
//! static_types/README.md`),所以增删夹具不需要改本文件。
//!
//! 三条断言里有一条是**按需**的:只有 `mode: both` 的控制组要求程序
//! `wlwl run` 跑得通。故意违例的夹具不要求 —— 静态类型违例在运行期多半
//! **也**是 `E0030`,那是 E0030 存在的意义,不是夹具坏了。

use std::path::{Path, PathBuf};
use std::process::Command;

/// `impl/tests/conformance/`
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-cli")
        .join("tests")
        .join("conformance")
}

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    panic!("wlwl binary not found; run `cargo build --bin wlwl` first");
}

/// 一份夹具的契约。
struct Contract {
    /// 门开着时必须出现的码
    expects: Vec<String>,
    /// 默认档也必须干净
    also_clean_when_off: bool,
}

/// 读夹具头部的两份注释。**不猜**:解析不出契约就让测试失败,免得夹具
/// 悄悄退化成「什么都不断言」。
///
/// [v0.10.1 / R10-011] 读**头部注释区**这个约定本身没错,错的是它当时承担了
/// 一份 `static_types.wll` 里 8 个断言的读取工作 —— 第一段之后全是死代码,
/// `E0111` / `E0112` / `E0116` / `W0117` 在夹具层面零断言,而守卫「至少读到
/// 一份契约」照样通过(它防的是「没有契约」,防不住「契约只读到第一段」)。
///
/// 修法是把夹具按本目录 README 的约定拆开:一份夹具一个子目录、一份契约。
/// 于是「读头部」与「读到全部」变成同一件事。
fn contract_of(source: &str) -> Contract {
    let mut expects = Vec::new();
    let mut also_clean_when_off = false;
    for line in source.lines() {
        let line = line.trim();
        if !line.starts_with("//") {
            // 契约必须在文件开头的注释区。
            if !line.is_empty() {
                break;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("// expects:") {
            expects = rest.split_whitespace().map(|s| s.to_string()).collect();
        }
        if line.trim_end() == "// mode: both" {
            also_clean_when_off = true;
        }
    }
    Contract {
        expects,
        also_clean_when_off,
    }
}

/// `static_types/` 下的夹具子目录(一份夹具一个目录,照本目录 README)。
fn static_type_fixture_dirs() -> Vec<String> {
    let root = fixture_root().join("static_types");
    let mut dirs: Vec<String> = std::fs::read_dir(&root)
        .expect("static_types fixture dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    dirs
}

/// `static_types/` 下的夹具目录数。
///
/// 这条常量是**元数据守卫**:加了夹具目录却忘了让它进断言覆盖面,这里就红。
///
/// 9 = 8 份 `expects:` 夹具 + `clean_baseline` / `exhaustive_result` 两个
/// `mode: both` 控制组。
///
/// [v0.10.1] 原先是 10,多出来的 `bounded_var_violation` 是 R10-025 的哨兵
/// (`T: A: B` 右嵌套)。它在这里待到了 R10-025 修完的那天,然后按设计
/// **变红** —— 因为修完之后 `T: Comparable: Integer` 成了 E0010 **解析错**,
/// 而本目录的夹具必须解析干净(见 README「为什么夹具必须解析干净」)。
/// 于是它移去了 probe 套件(`P_r10_025_nested_type_constraint`),那里才能
/// 断言解析错。
const EXPECTED_STATIC_FIXTURE_DIRS: usize = 9;

/// 把一份夹具复制成独立工程(`gradual_typing` 可选),返回入口路径。
///
/// 独立工程而不是就地跑:契约检查会沿 import 图找签名文件,夹具目录本身
/// 不该被跑测试的过程改动。
fn stage(fixture: &Path, name: &str, gradual: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("wlwl-conformance-static")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("staging dir");
    for entry in std::fs::read_dir(fixture).expect("fixture dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        // `README.md` 不是夹具。
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            continue;
        }
        let target = dir.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("fixture file copied");
        }
    }
    if let Some(level) = gradual {
        std::fs::write(
            dir.join("wlwl.toml"),
            format!(
                "[package]\nname = \"conformance\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = \"{level}\"\n"
            ),
        )
        .expect("manifest written");
    }
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("nested dir");
    for entry in std::fs::read_dir(from).expect("nested read") {
        let entry = entry.expect("nested entry");
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("nested file copied");
        }
    }
}

fn run_check(bin: &Path, dir: &Path, entry: &str) -> (bool, String) {
    let out = Command::new(bin)
        .arg("check")
        .arg(dir.join(entry))
        .output()
        .expect("wlwl check runs");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn run_program(bin: &Path, dir: &Path, entry: &str) -> (bool, String) {
    let out = Command::new(bin)
        .arg("run")
        .arg(dir.join(entry))
        .output()
        .expect("wlwl run runs");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// 一份夹具:**门开着报出期望的码** + **默认档一条都没有** + **程序本身能跑**。
fn assert_fixture(dir: &str, entry: &str) {
    let bin = wlwl_binary();
    let src = std::fs::read_to_string(fixture_root().join(dir).join(entry))
        .unwrap_or_else(|e| panic!("cannot read fixture {dir}/{entry}: {e}"));
    let contract = contract_of(&src);
    assert!(
        !contract.expects.is_empty() || contract.also_clean_when_off,
        "{dir}/{entry} declares neither `expects:` nor `mode: both` — \
         a fixture that asserts nothing is worse than no fixture"
    );

    // 1. 默认档(无清单):一条静态诊断都不能有,且必须通过。
    let off = stage(&fixture_root().join(dir), &format!("{dir}-off"), None);
    let (ok, out) = run_check(&bin, &off, entry);
    assert!(
        ok,
        "{dir}/{entry} must pass `wlwl check` by default:\n{out}"
    );
    for code in &contract.expects {
        assert!(
            !out.contains(code.as_str()),
            "{dir}/{entry} leaked {} with the static layer OFF:\n{out}",
            code
        );
    }

    // 2. 门开着:期望的码必须出现。
    let on = stage(
        &fixture_root().join(dir),
        &format!("{dir}-on"),
        Some("error"),
    );
    let (ok, out) = run_check(&bin, &on, entry);
    for code in &contract.expects {
        assert!(
            out.contains(code.as_str()),
            "{dir}/{entry} must report {} with gradual_typing=error:\n{out}",
            code
        );
    }
    // 声明 `mode: both` 的夹具:门开着也必须干净。
    if contract.also_clean_when_off {
        assert!(
            ok,
            "{dir}/{entry} declares `mode: both` but `check` failed with the layer on:\n{out}"
        );
    }

    // 3. 控制组(`mode: both`)还必须**真的能跑**。
    //
    //    故意违例的那几份**不**要求能跑:静态类型违例在运行期多半**也**是
    //    `E0030` 类型错误(那正是 E0030 存在的意义),要求它们 run 通过会
    //    把「静态层抓的类型错误」和「程序本身能跑」混成一件事。
    //    控制组承担另一半责任:它证明这套夹具与这套断言不是摆设 ——
    //    声明「两种模式都干净」的那两份,必须一次跑通。
    if contract.also_clean_when_off {
        let run = stage(&fixture_root().join(dir), &format!("{dir}-run"), None);
        let (ok, out) = run_program(&bin, &run, entry);
        assert!(
            ok,
            "{dir}/{entry} declares `mode: both`, so it must also RUN clean in \
             the default configuration:\n{out}"
        );
    }
}

/// §10.4 规划的 `static_types/` 目录。
///
/// [v0.10.1 / R10-011] 枚举**全部**子目录,而不是点名一个文件。v0.10 只有
/// `assert_fixture("static_types", "static_types.wll")` 一条断言,对着一个
/// 装了 8 个 `// expects:` 块的文件 —— 而 `contract_of` 读到第一段就 `break`,
/// 于是 7 条断言是死的。现在一份夹具一个子目录,加目录即加覆盖。
#[test]
fn static_types_fixtures_diagnose_only_when_the_gate_is_on() {
    let dirs = static_type_fixture_dirs();
    assert_eq!(
        dirs.len(),
        EXPECTED_STATIC_FIXTURE_DIRS,
        "static_types/ should hold exactly {EXPECTED_STATIC_FIXTURE_DIRS} fixture \
         dirs; a new fixture must be counted here so CI actually covers it"
    );
    for dir in &dirs {
        assert_fixture(&format!("static_types/{dir}"), "main.wll");
    }
}

/// [R10-011] 守恒断言:夹具声明的期望码集合必须覆盖静态层的全部硬诊断。
///
/// v0.10 的问题是 `E0111` / `E0112` / `E0116` / `W0117` **零断言** ——
/// 实现里写错了也绿。这里把「哪几个码必须有人守」钉死,少一个就红。
#[test]
fn static_types_fixtures_cover_every_diagnostic_they_claim_to() {
    let mut covered: Vec<String> = Vec::new();
    for dir in static_type_fixture_dirs() {
        let src = std::fs::read_to_string(
            fixture_root()
                .join("static_types")
                .join(&dir)
                .join("main.wll"),
        )
        .unwrap_or_else(|e| panic!("cannot read static_types/{dir}/main.wll: {e}"));
        covered.extend(contract_of(&src).expects);
    }
    for code in ["E0110", "E0111", "E0112", "E0116", "W0117"] {
        assert!(
            covered.iter().any(|c| c == code),
            "no static_types fixture asserts {code}; covered so far: {covered:?}"
        );
    }
}

/// [R10-023] 跨模块调用的实参必须按**旁路签名**检查。
///
/// 修之前导入的名字一律落 `Dynamic`(`Expr::Import` 那条臂直接返回
/// `Ty::Dynamic`),签名的**类型**只被解析、被展示,从不参与判定:
/// `math.wll.sig` 写着 `add (INTEGER, INTEGER) : INTEGER`,消费方
/// `add("a", "b")` 却零诊断。`module_sig` 原有的用例只验「名字在不在声明面里」
/// (E0113),签名**类型**这一面零覆盖。
#[test]
fn an_imported_signature_type_is_enforced_at_the_call_site() {
    let bin = wlwl_binary();

    // 开档:错实参必须报 E0111。
    let on = stage(
        &fixture_root().join("module_sig_import_type"),
        "module_sig_import_type-on",
        Some("error"),
    );
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        !ok,
        "a wrong argument at an imported call site must fail the gate:\n{out}"
    );
    assert!(
        out.contains("E0111"),
        "expected E0111 from the imported signature, got:\n{out}"
    );

    // 默认档:一条静态诊断都不许有(ADR-0020 S1)。
    let off = stage(
        &fixture_root().join("module_sig_import_type"),
        "module_sig_import_type-off",
        None,
    );
    let (ok, out) = run_check(&bin, &off, "main.wll");
    assert!(
        ok,
        "the default configuration must stay silent for this fixture:\n{out}"
    );
    assert!(
        !out.contains("E0111"),
        "an imported signature type must not leak into the default tier:\n{out}"
    );
}

/// [R10-023] 正面用例:实参**对得上**时跨模块调用必须干净。
///
/// 少了这条,R10-023 可能退化成「模块调用一律报错」—— 那同样是一种不可用。
#[test]
fn a_matching_argument_at_an_imported_call_site_stays_clean() {
    let bin = wlwl_binary();
    let on = stage(
        &fixture_root().join("module_sig_import_type"),
        "module_sig_import_type-good-on",
        Some("error"),
    );
    // 改入口文件(在**暂存副本**上,夹具目录不动):实参换成 INTEGER。
    std::fs::write(
        on.join("main.wll"),
        "IMPORT(\"math\", [\"add\"]);\nPRINT(add(1, 2));\n",
    )
    .expect("rewrite staged entry");
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        ok,
        "matching arguments at an imported call site must stay clean:\n{out}"
    );
}

/// [R10-011] **负向守卫**:契约写在代码之后,必须被判定为「什么都没断言」。
///
/// 这条是给守卫本身上锁。v0.10 的 `contract_of` 有注释说「不猜:解析不出契约
/// 就让测试失败」,但第一个块解析成功,守卫就通过了 —— 它防的是「没有契约」,
/// 防不住「契约只读到第一段」。这个用例把「只读到第一段」的后果钉成红灯:
/// 将来谁再把多段契约塞进一份夹具,这里立刻会拦住。
#[test]
fn a_contract_written_after_the_code_is_not_a_contract() {
    let misplaced = "// 这是一段说明。\nPRINT(1);\n// expects: E0110\n";
    let contract = contract_of(misplaced);
    assert!(
        contract.expects.is_empty() && !contract.also_clean_when_off,
        "a contract below the first code line must NOT be picked up: {:?}",
        contract.expects
    );
    // 头部那份则必须被读到 —— 否则上面这条断言会因为「什么都没读到」而空过。
    let well_placed = "// expects: E0110\nPRINT(1);\n";
    assert_eq!(contract_of(well_placed).expects, vec!["E0110".to_string()]);
}

/// §10.4 规划的 `module_sig/` 目录:签名与实现**一致**时零诊断。
#[test]
fn a_module_signature_that_matches_its_implementation_is_silent() {
    let bin = wlwl_binary();
    let on = stage(
        &fixture_root().join("module_sig"),
        "module_sig-on",
        Some("error"),
    );
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        ok,
        "a consistent module signature must not produce any diagnostic:\n{out}"
    );
    let run = stage(&fixture_root().join("module_sig"), "module_sig-run", None);
    let (ok, out) = run_program(&bin, &run, "main.wll");
    assert!(ok, "the module_sig fixture must run:\n{out}");
}

/// 破坏一致性时**确实**报出来(逐条)—— 否则上面那条「一致时静默」可能
/// 只是因为契约检查根本没跑。改动在**临时副本**上做,夹具目录不动。
#[test]
fn breaking_the_signature_is_reported_in_both_directions() {
    let bin = wlwl_binary();
    let dir = stage(
        &fixture_root().join("module_sig"),
        "module_sig-broken",
        Some("error"),
    );

    // E0113:实现多导出了签名没声明的名字。
    std::fs::write(
        dir.join("math.wll"),
        "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
         LET(PI, 3);\n\
         LET(secret, 4);\n\
         EXPORT([\"add\", \"PI\", \"secret\"]);\n",
    )
    .expect("patch math.wll");
    let (_, out) = run_check(&bin, &dir, "main.wll");
    assert!(
        out.contains("E0113"),
        "an export the signature does not declare must be E0113:\n{out}"
    );

    // E0114:签名声明了实现没导出的名字。
    std::fs::write(
        dir.join("math.wll.sig"),
        "EXPORT add (INTEGER, INTEGER) : INTEGER\nEXPORT PI : INTEGER\nEXPORT ghost : INTEGER\n",
    )
    .expect("patch signature");
    let (_, out) = run_check(&bin, &dir, "main.wll");
    assert!(
        out.contains("E0114"),
        "a declared-but-unexported name must be E0114:\n{out}"
    );

    // E0115:签名类型与实现注解冲突。
    std::fs::write(
        dir.join("math.wll.sig"),
        "EXPORT add (INTEGER, INTEGER) : STRING\nEXPORT PI : INTEGER\n",
    )
    .expect("patch signature");
    let (_, out) = run_check(&bin, &dir, "main.wll");
    assert!(
        out.contains("E0115"),
        "a signature type that conflicts with the annotation must be E0115:\n{out}"
    );
}

// ─────────────────────────────────────────────────────────────────────
// R10-073 · `SEALED` 密封面的端到端夹具
// ─────────────────────────────────────────────────────────────────────

/// [R10-073] `SEALED` 只有 `wlwl-types/src/sig.rs` 里的 **Rust 单测**,
/// 没有任何一份走 CLI 的夹具。
///
/// `sealed_surface/` 刻意做成**只有 SEALED 能逮到**的形状:签名里
/// **声明了** `secret`,密封面里没有。所以这一条验的不是「签名有没有覆盖
/// 导出面」(那是 `the_module_contract_violations_are_all_reported` 的活,
/// E0113 / E0114 / E0115 都归它),而是**密封面自己**有没有被强制执行。
///
/// 四个格子,少一个都不能证明「是 SEALED 抓的」:
///
/// | 格子 | 期望 | 证明什么 |
/// |---|---|---|
/// | 开档 + 密封面缺 `secret` | `E0113` | 密封面**被**执行 |
/// | 开档 + 密封面补上 `secret` | 零诊断 | 不是「见 SEALED 就报」 |
/// | 开档 + **删掉** `SEALED` 那一行 | 零诊断 | 不是签名抓的 |
/// | 默认档(无清单) | 零诊断 | ADR-0020 S1 |
#[test]
fn r10_073_a_sealed_surface_that_undershoots_the_real_exports_is_reported() {
    let bin = wlwl_binary();
    let fixture = fixture_root().join("sealed_surface");
    assert!(
        fixture.is_dir(),
        "conformance/sealed_surface must exist — without it `SEALED` has no \
         CLI-level coverage at all, only Rust unit tests"
    );
    let mod_wll = std::fs::read_to_string(fixture.join("mod.wll")).expect("read mod.wll");
    let sealed_line = "SEALED([\"add\"]);\n";
    assert!(
        mod_wll.contains(sealed_line),
        "the fixture must keep its undershooting SEALED line; got:\n{mod_wll}"
    );

    // ① 开档:密封面缺 secret → E0113。
    let on = stage(&fixture, "sealed_surface-on", Some("error"));
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        !ok && out.contains("E0113"),
        "an export missing from the SEALED surface must be E0113:\n{out}"
    );
    assert!(
        out.contains("SEALED surface"),
        "the diagnostic must name the SEALED surface specifically, otherwise we \
         cannot tell it apart from the signature-side E0113:\n{out}"
    );

    // ② 密封面补上 secret → 零诊断(否则这条只是「见 SEALED 就报」)。
    std::fs::write(
        on.join("mod.wll"),
        mod_wll.replace(sealed_line, "SEALED([\"add\", \"secret\"]);\n"),
    )
    .expect("widen the seal");
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        ok,
        "a SEALED surface that covers every export must be silent:\n{out}"
    );

    // ③ 删掉 SEALED 那一行 → 零诊断。签名声明了 secret,所以签名这一路是
    //    干净的;不干净就说明这条 E0113 其实另有来源。
    std::fs::write(on.join("mod.wll"), mod_wll.replace(sealed_line, "")).expect("drop the seal");
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        ok,
        "the same code without SEALED must be silent — the fixture only proves \
         something if the seal is the sole cause:\n{out}"
    );

    // ④ 默认档:零诊断且程序能跑(ADR-0020 S1)。
    let off = stage(&fixture, "sealed_surface-off", None);
    let (ok, out) = run_check(&bin, &off, "main.wll");
    assert!(ok, "the default tier must stay clean:\n{out}");
    let (ok, out) = run_program(&bin, &off, "main.wll");
    assert!(ok, "the sealed_surface fixture must also run:\n{out}");
}

// ─────────────────────────────────────────────────────────────────────
// R10-074 · `match_exhaustiveness` 开关的**独立性**
// ─────────────────────────────────────────────────────────────────────

/// [R10-074] `match_exhaustiveness` **缺省跟随** `gradual_typing`(spec §9.4),
/// 而 `static_types/` 全部夹具走的都是「只写 `gradual_typing = "error"`」
/// 那一条路 —— 于是「这个开关自己开着的时候管不管用」**从未被验过**。
///
/// 实测行为是对的(下面这张表就是实测值),缺的是把它钉住。
///
/// 判据用**一份同时踩两个头**的程序:非穷尽 `MATCH`(该 `E0116`)+
/// 注解不匹配(该 `E0110`)。两个码在同一个输出里,才能证明两个开关
/// **互相独立**,而不是「一起被同一个总开关带着走」。
#[test]
fn r10_074_match_exhaustiveness_is_independent_of_gradual_typing() {
    const PROG: &str = "LET(r, ERR(\"boom\"));\n\
                         LET(out, MATCH(r, [[OK(v), \"ok\"]]));\n\
                         PRINT(out);\n\
                         LET(flag: BOOLEAN, \"yes\");\n\
                         PRINT(flag);\n";

    fn codes(text: &str) -> Vec<String> {
        let mut v: Vec<String> = text
            .lines()
            .filter_map(|l| {
                let i = l.find('[')?;
                let j = l[i..].find(']')? + i;
                Some(l[i + 1..j].to_string())
            })
            .filter(|c| c.starts_with('E') || c.starts_with('W'))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    fn check(features: Option<&'static str>) -> (bool, String) {
        let bin = wlwl_binary();
        let dir = std::env::temp_dir().join("wlwl-switch-matrix");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("staging dir");
        std::fs::write(dir.join("main.wll"), PROG).expect("write program");
        if let Some(features) = features {
            std::fs::write(
                dir.join("wlwl.toml"),
                format!(
                    "[package]\nname = \"sw\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n{features}"
                ),
            )
            .expect("write manifest");
        }
        // 无清单时**不写** wlwl.toml —— 写一个只有 [package] 的也行,但那样
        // 测的就不是「没有清单」这条路径了。
        run_check(&bin, &dir, "main.wll")
    }

    /// 矩阵的一格。四元组太长,clippy 会叫 —— 拆成具名字段,失败信息才读得懂。
    struct Cell {
        /// `[features]` 段的内容;`None` = 不写清单。
        features: Option<&'static str>,
        /// 必须出现的码
        want: &'static [&'static str],
        /// 必须**不**出现的码
        unwanted: &'static [&'static str],
        /// 这一格证明什么(进失败信息)
        why: &'static str,
    }

    const E0110: &str = "E0110";
    const E0116: &str = "E0116";

    let cells = [
        Cell {
            features: None,
            want: &[],
            unwanted: &[E0110, E0116],
            why: "没有清单 = 静态层整条不执行(ADR-0020 S1)",
        },
        Cell {
            features: Some("[features]\ngradual_typing = \"error\"\n"),
            want: &[E0110, E0116],
            unwanted: &[],
            why: "只开 gradual:MATCH 头靠缺省跟随一起开",
        },
        Cell {
            features: Some("[features]\nmatch_exhaustiveness = \"error\"\n"),
            want: &[E0116],
            unwanted: &[E0110],
            why: "**只开 match**:E0116 独立生效,且没有顺手带上 E0110",
        },
        Cell {
            features: Some(
                "[features]\ngradual_typing = \"error\"\nmatch_exhaustiveness = \"off\"\n",
            ),
            want: &[E0110],
            unwanted: &[E0116],
            why: "**显式关掉 match**:E0116 必须消失,E0110 留着",
        },
        Cell {
            features: Some(
                "[features]\ngradual_typing = \"off\"\nmatch_exhaustiveness = \"error\"\n",
            ),
            want: &[E0116],
            unwanted: &[E0110],
            why: "反向:gradual 关、match 开,只剩 E0116",
        },
        Cell {
            features: Some(
                "[features]\ngradual_typing = \"off\"\nmatch_exhaustiveness = \"off\"\n",
            ),
            want: &[],
            unwanted: &[E0110, E0116],
            why: "两个都关 = 干净",
        },
    ];

    for cell in &cells {
        let label = cell.features.unwrap_or("(无清单)").replace('\n', " ");
        let (ok, out) = check(cell.features);
        for code in cell.want {
            assert!(
                out.contains(code),
                "[{label}] expected {code} — {}\n{out}",
                cell.why
            );
        }
        for code in cell.unwanted {
            assert!(
                !out.contains(code),
                "[{label}] must NOT report {code} — {}\n{out}",
                cell.why
            );
        }
        // 只要这一格期望零诊断,就必须真的 rc=0;期望有码就必须 rc!=0。
        let expect_clean = cell.want.is_empty();
        assert_eq!(
            ok,
            expect_clean,
            "[{label}] rc mismatch (clean={expect_clean}) — {}; codes={:?}\n{out}",
            cell.why,
            codes(&out)
        );
    }
}
