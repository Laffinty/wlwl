# WLWL v0.11.3 初次审查报告(审计 · 用于执行修复)

| | |
|---|---|
| **对象** | 分支 `wip0.11.3` @ `b01ea41`(工作树 clean),覆盖主计划 M0–M4 与追加批次 M5–M8 全部 15 份方案 |
| **方法** | ① 六项门禁全量实跑;② 三个专项深审(sanitize / regex / time·net·process·fs·compress)逐行读源码 + 契约测试,关键发现逐条回源码复核;③ 密码学面(encode 全文 + rand 抽样)、json/math/result、R0 归层 kernels、横切一致性(完成度声明 / 数字口径 / ADR 状态 / 双向锁)由审查者亲自核查。**本报告所有 P0 与 P1 均已在源码行级复核过**,不是转述 |
| **门禁实测(2026-10-15)** | `fmt --check` 0 diff;`clippy --all-targets -D warnings` 0;`deny check` 全绿;`doc -D warnings` 0 error;`check -p wlwl-std --features real-ai` exit 0;`test --locked --all-targets` **出现 1 次失败后复跑 10+ 次全绿**(见 N-012)。附录 A 生成器重生成**零 diff(双向锁绿)**;probe 目录数 **175**(与 README 一致);成员面 **214**(附录 A 头部,与 addendum-12 复核一致) |
| **性质** | 初次审查(review-1)。发现按 P0(阻断发版)/ P1(应修)/ P2(建议)/ P3(微瑕)分级;编号 N-001 起,供修复批次引用 |

---

## 0. 总评

**架构与工程纪律:优秀,达标。** 树构建净化器(否决 tokenizer)、Pike VM 线性正则、KDF 参数硬边界、契约期望值独立裁定(Python/官方向量交叉)、偏差登记的诚实度(D13-001~018)都经得起核。**「设计目标已实现」的主体声明成立** —— 附两个事实性保留:① **`std.sync` 0/5 受阻**(addendum-12 ⛔,硬前置 `ADR-0017` Step 3 未落地,文档已如实登记,不是隐瞒);② addendum-14 按 §6 **如实记为「部分完成」**(R2 原生成员的官方包分发形态缺口)。

**但本轮抓到 3 个 P0 与 11 个 P1。** 三个 P0 全部是**静默给错结果**的安全/正确性缺陷,且都活在现有测试的盲区里(这正是本仓最依赖的「契约测试反向钉住」纪律的边角失效);多数 P1 有共同根因:**「守卫钉了常量/文档,没钉行为路径本身」**与**「Windows 修过、POSIX 结构上测不到」**。建议:发 tag 前修完 P0 与 P1 中的 N-004/N-005/N-006/N-007(正确性与崩溃级);其余可并入下一批次,但必须在 §9 偏差台账登记。

---

## 1. P0 —— 阻断发版

### N-001 · `strip_comments: false` 下,注释可经 `<!-->` 塌缩逃逸成活标记(mXSS)
- **位置**:`impl/crates/wlwl-std/src/sanitize/serialize.rs:34-38` + `tokenize.rs:125-137`。
- **触发链**(已逐字节推演):输入 `<!--><img src=x onerror=alert(1)>`;tokenizer 找不到 `-->` ⇒ 整个余文成为 Comment 数据;serialize 输出 `<!--` + 数据 + `-->` ⇒ **输出串含 `<!-->`**,而 WHATWG 把 `<!-->` / `<!--->` 规定为**空注释**(abrupt closing)⇒ 浏览器重解析时 `<img …>` 落在注释外成为活标记 ⇒ XSS。
- **为什么现有防线抓不住**:自幂等成立(二次 sanitize 解析回同一 Comment),W-12 三组幂等测试全绿 —— 本条**证伪了「自幂等 ⇒ 浏览器重解析安全」的方法论假设**,幂等性是必要不充分条件。
- **触发条件**:`policy["strip_comments"] = FALSE`(规范性策略键,有冻结契约 `policy_keep_comments`,只是用例只喂了良性注释)。
- **修法方向**:serialize 时若剥除 `--` 后的数据以 `>` 或 `->` 开头,前置空格或强制丢弃;补 `<!-->` / `<!--->` × `strip_comments=false` 冻结契约;**并把「输出按 WHATWG 注释规则再判读」补进 W-12 方法论**(哪怕只是手工裁定的冻结用例)。

### N-002 · regex 字符类内 `[^\d]` / `[^\w]` / `[^\s]` 整类反转(补集施加两次)
- **位置**:`impl/crates/wlwl-std/src/regex.rs:538-541`(类内预定义类)+ `:571-573`(整类收尾)。
- **代码**:`let sub = if negated { sub.complement() } else { sub };` —— 而 538 行上方注释自己写着「否定情形由整类的 `^` 统一处理」,与代码直接矛盾(残留代码)。`[^\d]` ⇒ 先取补并入、收尾再对整类取补 ⇒ 结果 = `\d` 本身。**`RE_SEARCH(RE("[^\w]"), "a b")` 匹配 `"a"` 而不是 `" "`**。`[^\D]` / `[^a-z\d]` 同病。
- **测试盲区**:单测与契约只有 `[^a-c]`(纯字面,正确),没有一条 `[^<预定义类>]`。
- **修法方向**:删掉该三元(一行),直接 extend;补 `[^\d]` 契约用例(期望值与 Python 对拍)。

### N-003 · regex 贪婪匹配在 ~65 码点处静默截断(`PRIO_MAX` 封顶 + 抵达顺序比较方向写反)
- **位置**:`regex.rs:919-937`(封顶)、`:845-850`(`Hit::better_than`)、`:1002-1029`。
- **推理**:封顶后第 ≥65 次贪婪延伸的候选 `prio` 全部塌缩成同一向量,取舍落到第三键 `arrival` —— 而 `better_than` 是**升序**(早到者胜),于是 `RE_SEARCH(RE("a+"), "a"*100)` 匹配 **65** 个 a,不是 100。`Hit` 结构注释(`:841`)与 PRIO_MAX 注释(`:916-918`)都断言「同 `(start, prio)` 时**后到者**更完整」—— **注释与比较方向相反,addendum-02 W-04 的「封顶不损语义」论证被推翻**。`RE_FIND_ALL` 有「同起点取更长」逻辑碰巧自愈;`RE_SEARCH` / `RE_REPLACE` 没有。
- **测试盲区**:契约用例最长 run 7 字符,基准 8 字符,全部够不到 64 门槛。
- **修法方向**:`better_than`(及 `find_all` 排序键)的 `arrival` 改**降序**(封顶前 arrival 从不参与裁决,封顶后「后到 = 消耗更多 = 贪婪更优」,恰与注释宣称一致);补 `REPEAT("a",100)` 级契约用例。**注意 N-004 与本条是同一处手术**。

---

## 2. P1 —— 应修(正确性 / 崩溃 / 验收未落地)

### regex(与 N-002/N-003 同模块)
- **N-004 · `find_all` 的「同起点取代」破坏 leftmost-first**(`regex.rs:1154-1171`):`RE_FIND_ALL(RE("a|ab"), "ab")` 得 `["ab"]`,Python/Go 得 `["a"]`。该取代规则在贪婪延伸下永不触发(更长 prio 必更小),真正触发的两个场景(交替低优先支更长、N-003 封顶平手区)一概错杀。`RE_REPLACE` / `RE_SPLIT` 建在其上,连带受影响。修法:结合 N-003 的 arrival 降序,封顶平手区在排序里自然「最长在前」,取代分支删除或收紧为「仅 `prio` 相等时允许」。
- **N-005 · 嵌套深度无上限 ⇒ 栈溢出是 abort 不是 E0030**(`regex.rs:291-576` 解析递归、`:647-747` 编译递归、`Ast` Drop 递归):2 万层括号模式击穿线程栈,**直接带走解释器进程**(abort 不可捕获),违背「语法错 → E0030 优雅中止」的失败分界。编译期有 `MAX_REPEAT` / `MAX_PROG` 两道闸,唯独缺深度这道。修法:解析器加深度计数(超限 E0030 「pattern nests too deeply」,Go `regexp/syntax` 同款)。

### sanitize(与 N-001 同模块)
- **N-006 · 未匹配结束标签 → O(n²)**(`sanitize/tree.rs:148-152`):`stack.iter().rposition` 找不到时扫全栈;`"<b>"*50_000 + "</i>"*50_000` ≈ 25 亿次比较。规范 §13.2 把「线性输入不得超线性」写成**规范性条款**,病态契约只测了配对嵌套恰好测不到。修法:每标签名维护栈内最新位置,O(1) 判负。
- **N-007 · raw-text 关闭扫描每元素物化整个余文的小写副本 → O(n·k) 内存流量**(`sanitize/tokenize.rs:81`):`src[next..].to_ascii_lowercase()` 对 8 种 raw-text 元素逐个触发;1 MB 输入 + 1 万个 raw-text ⇒ GB 级拷贝。修法:原位 `eq_ignore_ascii_case` 滑窗。

### std.net
- **N-008 · URL 端口溢出 → 宿主 panic**(`net.rs:425`):`p.parse().expect("all digits")` —— 数字串可超 `u32::MAX`,`URL_PARSE("http://a:99999999999999/")` **纯输入打死解释器进程**。契约只钉了 `70000`,没钉 11 位以上。修法:`parse().ok()` 失败或 `>65535` 一律走既有 exceeds 分支。

### std.time
- **N-009 · 已结束 `TIMEOUT` 的 deadline 永留候选集 → 其后任何 `SLEEP` 被幽灵超时劫持**(`wlwl-eval/src/lib.rs:6479-6484` + `time.rs:270`):`pop_deadline` 只复位 `tripped`,**条目从不出栈**;同一气泡内 `TIMEOUT` 结束后再 `SLEEP(1)` 也会 trip 死 deadline 返回 `ERR Timeout`,把 `TEST_BUBBLE` body 弹断。现有契约恰好在「弹完不再 SLEEP」的形状下测,全绿。修法:`swap_remove`/`drain` 真正移除(严格嵌套保证栈纪律);补「TIMEOUT 之后同气泡 SLEEP 不受影响」契约。

### std.fs / std.process(POSIX 平台盲区)
- **N-010 · `WALK_DIR` 的 `follow=true` 在 POSIX 上是死代码;符号链接验收条款整体未落地**(`fs.rs:439`):`DirEntry::file_type()` 不跟随符号链接 ⇒ Unix 上链接→目录的 `is_dir()` 恒 false,`follow` 开关无效;Windows 上 dir-symlink 属性位又带 DIRECTORY,**同段代码跨平台语义分叉**。全仓零符号链接用例,addendum-06 §4 点名的「成环链接目录必须终止」验收**没有对应测试**(visited 防环逻辑零覆盖)。修法:「可下降」判定改 `path.is_dir()`(仅 `follow` 时用);补成环终止契约。
- **N-011 · 超时路径管道收尾仍无限期阻塞(POSIX),代码与自己的注释矛盾**(`process.rs:78-88`):注释说「仍未结束就**丢弃句柄**让它自生自灭」,代码 `already_timed_out=true` 时**无条件 `join()`**;Windows 被 `taskkill /T /F` 掩盖,POSIX 上杀掉 `sh` 后孤儿 `sleep` 握着管道写端 ⇒ 超时路径拖满子进程时长(与 addendum-06 W-05 修掉的 29.26 s 缺陷同构,平台换位)。回归测试在 POSIX 用无孙进程的直接子进程,结构上抓不到。修法:两路径统一「有界轮询 → 未结束 `drop(h)` 返回空」;POSIX 补孙进程超时用例。

### std.compress
- **N-012 · flate2 家族缺省级别 = 0 =「不压缩」,与规范「缺省库默认(6)」相悖**(`compress.rs:118-139`、`:279/298/321`):`Compression::new(0)` 是 stored 块;反证就在自家档案里 —— `compress_contract.rs:230-234` 与 `baseline.txt` 段十四记的「104 字节压到 115(比原文大)」正是 level 0 的形状。zstd 侧 `0` 恰好映射库默认所以没炸。修法:缺省改 `flate2::Compression::default()`;补「缺省级别下高冗余输入必须变小」契约。**注意:这是修 bug,不是冻结错误输出 —— 沿 D13-008 先例(「已改实现而非冻结错误输出」),契约里若冻了 stored 输出须同步改判并注明。**

### std.json
- **N-013 · `JSON_CHECK` 的 schema 解析与校验是无界递归**(`wlwl-std/src/json/schema.rs:56` `parse_schema` 经 `properties`/`items` 递归、`:205` `check` 同):wlwl 程序可**迭代**构造 10 万层嵌套的 schema DICT(或被检值),`JSON_CHECK` 递归下去 ⇒ 栈溢出 abort。同文件 `lexer.rs` 的 `MAX_DEPTH=128` 证明深度闸在本模块是既有范式,schema 侧漏了。修法:`parse_schema` 与 `check` 各加深度上限(复用 128 口径,超限 `E0030`)。

### 门禁自身
- **N-014 · 全量测试出现 1 次无法归因的 flaky 失败**:`cargo test --locked --all-targets` 首轮在 `-p wlwl-std --lib`(283 条)报 1 failed,单跑与并发压测(6 实例 × `--test-threads 2`,正确 cwd)**10+ 次未复现**;首轮日志经 grep 过滤未保留失败测试名(本次审查的操作教训:失败详情不该被过滤掉)。嫌疑:encode 的时序分布断言(`content_does_not_change_the_timing` 等 min 采样,`encode.rs:1351-1386`)与 `net::tests::https_and_connection_refused_are_err_values`(外部网络依赖)。修法方向:① 给时序断言加大样本/放宽阈值或标记 `ignore`;② net 测试改为本地回环夹具;③ CI 侧保留失败名再复跑一次的机制。

---

## 3. P2 —— 建议(语义差分 / 文档-代码不符 / 资源)

**sanitize**(`sanitize/`):
- N-015 `<` + 数字在串中段造假标签吞后续文本(`tokenize.rs:69` 用 `is_ascii_alnum`,WHATWG 要求 `is_ascii_alphabetic`;`3<4 and 5<6` → `3&lt;6`,`"4 and 5"` 静默蒸发;契约 `malformed_lone_lt` 是碰巧绿)。
- N-016 相对 URL 含 `:` 被误判 scheme 拒掉(`filter.rs:218` 注释说「`:` 前有 `/ ? #` → 相对恒允许」,`:225-248` 代码没实现;`/wiki/Special:Search` 的 href 被删 —— 方向保守但违背自家规范 §13.2「相对 URL 恒允许」)。
- N-017 `HTML_SANITIZE_CSS` 拒绝扫描可被 CSS 转义绕过(`css.rs:142` 字面字节扫描 vs CSS 解码转义;`e\78 pression(...)` 通过;现代浏览器可利用性低,**待动态验证**)。
- N-018 `HTML_SANITIZE_CSS` 输出可含裸 `</style>`(嵌入 `<style>` 即破壳;`HTML_SANITIZE_SCRIPT` 有同等硬化而 CSS 没有,设计不一致。**待动态验证**)。
- N-019 属性值内 legacy 无分号实体也解码(HTML5 属性上下文后随 `=`/字母数字时不应解码;保守方向,登记差分即可)。
- N-020 filter 属性去重 O(k²)(`filter.rs:317-322`);`<a><a>` 嵌套 vs adoption agency 拍平、`<b / >` 自闭合差分 —— 两条未进 §13.2 差异登记清单。

**regex**:`\b \B \A \z \Z` 静默当字面量(N-021,应编译期拒收或实现 `\b`);`[a-\d]` 静默变 `a-d`(N-022,应报错);`RE_REPLACE` 的 `$5` 越界组静默展开空串而非承诺的 E0030(N-023);`find_all` hits 内存 O(n) + 排序 O(n log n)(N-024,对线性承诺的边角侵蚀);`(?i)` 作用域比 RE2/Perl 窄、`$` 不匹配结尾换行前 —— 两条 RE2/Go 语义分歧未写进文件头六条(N-025);每线程每步 `Inst` clone + `vec![start]` 的分配热点(N-026,不改架构可消)。

**time / net / process / fs / compress**:
- N-027 gzip 多成员 / 流后尾随字节**静默截断**(`compress.rs:307-311` 单成员 `GzDecoder`;换 `MultiGzDecoder` 或校验 `Cursor::position()` 吃尽)。
- N-028 64 MiB 解压炸弹护栏**零执行覆盖**(常量有守卫、路径没跑过;addendum-07 §3.2「1 MB→GB 断言 ERR」验收未兑现,至少造一个真超限用例)。
- N-029 `ENV_SET` 与 `PROCESS_RUN` 继承关系文档与实现相反(`env.rs:15/76` 说「子进程继承」,`process.rs:212-221` 从不合并 `ctx.env`;二选一:实现继承或改文档)。
- N-030 POSIX 上含 `\` 的合法文件名被 `spec()` 无条件改写为 `/`(`fs.rs:146-148`;且与 `native()` 只在 Windows 替换的口径打架)。
- N-031 `WALK_DIR` 对打不开的根静默返回 `[]`,同模块 `LIST_DIR` 返回 ERR —— 失败口径矛盾(`fs.rs:422`)。
- N-032 `MOVE` 回退的 `copy_tree` 跟随符号链接且无 visited ⇒ 链接环可打挂(`fs.rs:620-632`)。
- N-033 HTTP 超时不覆盖 DNS 解析;多地址连接超时可线性放大;读超时逐次重置(`net.rs:739-759`;建议总 deadline 全程卡住)。
- N-034 HTTP/1.0 默认 close 定界被误拒(`net.rs:828-830` 仍强制要求 `Connection: close` 或 `Content-Length`)。
- N-035 `tchar` 集合经 `is_sub_delim` 放进了 RFC 7230 不含的五个字符(`net.rs:91-111`);响应体非 UTF-8 整条 ERR,与 `READ_BYTES` 字节 doctrine 不一致(N-036)。
- N-037 子进程输出 `from_utf8_lossy` 有损替换未写进成员文档;POSIX 超时后孙进程泄漏;`try_wait` Err 分支不 `wait`(`process.rs:286-287`、`:252-256`)。
- N-038 `READ_BYTES` 无大小护栏,`ARRAY[INTEGER]` 每字节 ~16-30× 内存放大(`fs.rs:656-671`,「小载荷」条款只是文档)。

**横切 / 文档**:
- N-039 **addendum-03 与 addendum-05 的头部状态陈旧**:两份头部仍写「🚧 W-01/W-02(03 加 W-03)已结 —— W-04/W-03 起」,而 §7 检查点表已 5/5 全 ✅(03 到 2026-10-11、05 到 2026-10-11)。`dd82955`「9 份 addendum 头部状态与实际结项对齐」漏掉了这两份。
- N-040 README(2026-10-05)「当前状态」行的长段落仍描述 M6 中途态(「`std.time` A1 已落地…probe 164」),与同文件门禁行(2026-10-15 实测)冲突 —— 同页两个时间口径的快照并存,建议整段重写到当前态。

## 4. P3 —— 微瑕(择要)

- sanitize:`tree.rs`/`serialize.rs`/`css.rs`/`entities.rs` 零文件内单测(W-11 写的是五个文件各配单测,实际只有 tokenize/filter 有 —— 由契约测试兜住);`ExitOwned` 每元素 clone、`decode` 无 `&` 快路径缺失、css 两处 `Vec<char>` ×4 内存;计划写 `u32` 句柄实现是 `usize`;`filter.rs:217-219` doc 两行重复;变异集 324 条只是 println,硬守卫 `>=150`(D13-007 已登记,维持)。
- regex:`advance`/`finish` 的 `Inst::Assert` 分支死代码;`a**` 报「nothing to repeat」(应为 multiple repeat);`gen: u32` 理论回绕;`groups[0]` 含整体匹配与 Python 习惯不同(已裁决,建议规范点名)。
- 审查中确认**无恙**的高危面(正向结论,供修复时不必重查):实体全表程序化对账全过(2125+106 项、排序二分、windows-1252 五洞、数字引用饱和防溢出);序列化文本/属性均过 `escape_str`,输出无无分号实体;`javascript:` 判定全族向量有冻结契约;panic 面未发现 unwrap/越界/溢出;`hmac_sha256_bytes`(RFC 2104)、`pbkdf2_hmac_sha256`(块计数器/异或链)、`timing_safe_eq_bytes`(无分支、长度差折入累加器)逐行核对正确;`rand.rs` 的 `unbiased_with` 是正确的 u128 拒绝采样;归层 kernels 的 D 类键用 `display()` 串**结构性地避开了** `f64` 哈希坑(D13-015 教训已内化),`UNIQ` 平方级残余的登记与理由属实;`MODE` 的 `mode_key` 有符号零修复在位;json lexer 深度闸 128 在位。

## 5. 设计目标达成度(逐里程碑)

| 里程碑 | 判定 | 依据 |
|---|---|---|
| M0 文档先行 | ✅ | §13.3/§14/警示句/标签残留全核实;零实现代码 |
| M1/M2 转义 + 哈希 | ✅ | FIPS/RFC 4231 逐字 + Python 交叉核对;基准达标;D13-001/002 已钉 |
| M3 `HTML_SANITIZE` | ⚠ **主体达标,安全面有 P0** | 树构建/幂等三组/81 契约/病态断言/基准全在位;但 N-001 证明幂等方法论有缺口,N-006/N-007 破线性承诺(规范性条款) |
| M4 收口 | ✅ | 九处表/CHANGELOG/版本号(11 处 0.11.3)/probe 157 核对一致 |
| M5 L0 归层 | ✅ | 13 成员归 R2、契约 127 条零改动机械核验、回归 84 条;UNIQ/SORT 残余登记属实 |
| M6 L1 八份 | ⚠ 大体达标 | 01/02/04/07/08 高质量;**05 net 有 N-008 panic**、**03 unicode 头部状态陈旧(N-039)**;02 regex 有 N-002~N-005 四条(静态正确性,非性能) |
| M7 L2 五份 | ⚠ | 09/10/11/13 结项属实;**12 std.sync 0/5 受阻属实且登记诚实**;09 json 有 N-013 |
| M8 L3 减法 | ✅(如实部分完成) | ADR-0034 Accepted;形态缺口按 §6 登记 |

**「所有构建任务已完成」的口径更正**:15 份方案 83 工作项中 78 项完成;`std.sync` 受阻与 addendum-14 部分完成均为**显式登记的在案状态**,不是遗漏,但对外表述应保留这两条保留项。

## 6. 修复执行建议(供排期)

1. **发 tag 前必修**:N-001、N-002、N-003(+N-004 同批手术)、N-005、N-008、N-009、N-013(全部是静默错误结果或进程崩溃);N-014 至少完成归因。
2. **随修复批一并**:N-006、N-007(线性承诺是规范性条款)、N-010、N-011、N-012。
3. **登记入 §9 偏差台账后择批**:P2 全部(建议逐条沿用 D 编号续篇 D13-019 起),其中 N-017/N-018 标注「待动态验证」。
4. **文档批(半天)**:N-039、N-040 + P3 的文档性条目。
5. **方法论补丁**:W-12 幂等套件增补「浏览器重解析差分」判读组(N-001 的教训);病态契约增补 N-006/N-007 两条语料形状 —— 守卫的扫描范围必须等于被承诺的行为。
