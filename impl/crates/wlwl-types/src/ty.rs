//! 静态类型 IR(ADR-0020 A1 · build plan Step 1)。
//!
//! **分层纪律**:本模块的 [`Ty`] 是**编译期**类型表示,与运行时 `TYPE(x)`
//! 的字符串比对路径(ADR-0010 的 transient cast)**严格分层**。
//! `wlwl-eval` 不依赖本 crate,运行时行为不受本模块影响(ADR-0020 S1/S3)。
//!
//! **语法面零扩张**:本模块只**消费** `wlwl-ast::TypeExpr` 的既有三形态
//! (`Ident` / `Array` / `Generic`),不改 AST。新类型名一律走
//! `Generic { name, args }` 既有槽位(ADR-0020 A1 设计约束)。
//!
//! **记法**:`Display` 用**方括号**(`ARRAY[INTEGER]`),与 parser 实际
//! 产出的 `TypeExpr` 一致,因此 `parse(display(ty)) == ty` 可往返。
//! build plan 正文用的尖括号记法(`ARRAY<T>`)只是散文简写,不是词法形式。

use std::fmt;

use wlwl_ast::TypeExpr;

/// 静态类型表示(ADR-0020 A1 最小集)。
///
/// 变体集合**刻意**等于 build plan §3.1「静态类型最小集」,不多不少:
/// 运行时存在但本层不结构化建模的类型(`TASK` / `CHANNEL` / `CLASS` /
/// `INSTANCE`)一律落到 [`Ty::Named`],而不是各开一个变体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// spec v0.9 §2.1 `INTEGER` — 有符号 64 位整数
    Integer,
    /// spec v0.9 §2.1 `FLOAT` — IEEE 754 双精度
    Float,
    /// spec v0.9 §2.1 `STRING`
    String,
    /// spec v0.9 §2.1 `BOOLEAN`
    Boolean,
    /// spec v0.9 §2.1 `NULL`
    Null,
    /// `ARRAY[T]` — 元素类型已知的数组
    Array(Box<Ty>),
    /// `DICT[K, V]` — 键值类型已知的字典(spec §2.1:键限 `STRING` / `INTEGER`)
    Dict(Box<Ty>, Box<Ty>),
    /// `OPTION[T]` — **仅注解侧**的可选包装。
    ///
    /// v0.9 运行时的「无值」用 `NULL` 或 `RESULT` 表达,没有对应的运行时
    /// 类型名(§2.1 的 13 个类型里没有 `OPTION`)。因此本变体是**注解糖**,
    /// 与运行时值的匹配规则由 A4 决定,本层不定。
    Option(Box<Ty>),
    /// `RESULT[T, E]` — `OK[T]` 折叠为 `Result[T, Dynamic]`,
    /// `ERR[E]` 折叠为 `Result[Dynamic, E]`
    Result(Box<Ty>, Box<Ty>),
    /// `FUN(T...) -> U` — 函数类型。
    ///
    /// **本版不可从 parser 到达**:`wlwl-ast/src/lib.rs:83-84` 把
    /// `FUN(...) -> T` 标为 "reserved for v0.4",至今未实现;lexer 也
    /// 没有 `->` 终结符(`-` 与 `>` 是两个独立 token)。本变体先落 IR,
    /// 供 A4 边界检查与 P1-2 限形泛型使用;接 parser 语法待决策点 D-5。
    Fun {
        /// 形参类型,按位置排列
        params: Vec<Ty>,
        /// 返回类型
        ret: Box<Ty>,
    },
    /// 已知但本层不结构化建模的具名类型。
    ///
    /// 覆盖两种情形:(1) 运行时真实类型但不在最小集内
    /// (`TASK` / `CHANNEL` / `CLASS` / `INSTANCE`);(2) 未知或用户自造
    /// 类型名。**保名保参**以便 A4 报出可诊断的失配,不做静默丢弃。
    Named {
        /// 原始类型名(不做大小写折叠,便于诊断回显源码)
        name: String,
        /// 类型参数;裸标识符为空
        args: Vec<Ty>,
    },
    /// 未标注处的回退类型 / 顶底元。
    ///
    /// 与任何类型双向可赋值(ADR-0020 A1:`Dynamic` 是顶/底元)。
    Dynamic,
    /// v0.10 Step 9(计划书 §5.2 P1-2,决策 D-5)**类型变量 + 显式约束**。
    ///
    /// 源码形状是 `T` / `T: Comparable`(见 [`wlwl_ast::TypeExpr::Bounded`]。
    /// 本版**只做擦除**:不 monomorphize、不做 let-多态、不做 trait 求解 ——
    /// 变量的类型在**调用点**被实参实例化,运行期什么也不剩。
    ///
    /// `bound` 为 `None` = 无约束变量(接受任何类型)。`Some(b)` = 该变量的
    /// 实参类型必须满足 `b`;`b` 自身只是个类型名
    /// (`Comparable` → [`Ty::Named`]),语义见 [`Ty::satisfies_bound`]。
    Var {
        /// 变量名
        name: String,
        /// 显式约束(无约束时为 `None`)
        bound: Option<Box<Ty>>,
    },
}

impl Ty {
    /// 从 `wlwl-ast::TypeExpr` 映射到静态类型。
    ///
    /// 大小写不敏感 —— 与运行时 `E0033` 路径的既有行为一致
    /// (`wlwl-eval/src/lib.rs:8587` 注释:case-insensitive, top-level
    /// shape only)。
    ///
    /// **不 panic、不返回 Result**:映射是全函数。识别得出的头 + 正确元数
    /// 落到具体变体;裸头(无类型参数)落到该头的 `Dynamic` 参数形式;
    /// 识别得出但**元数错误**的头**保留为 [`Ty::Named`]** 而非丢弃
    /// 参数,让 A4 能报出「元数不对」而不是「类型不认识」。
    pub fn from_type_expr(expr: &TypeExpr) -> Ty {
        match expr {
            TypeExpr::Array { element, .. } => Ty::Array(Box::new(Ty::from_type_expr(element))),
            TypeExpr::Ident { name, .. } => Ty::from_head(name, &[]),
            TypeExpr::Generic { name, args, .. } => {
                let mapped: Vec<Ty> = args.iter().map(Ty::from_type_expr).collect();
                Ty::from_head(name, &mapped)
            }
            // [v0.10 Step 9] 带约束的类型变量。
            //
            // 头名是**已知类型**时约束被丢弃:给具体类型加约束是无意义的
            // 废话(`INTEGER: Comparable` —— INTEGER 本来就可比较),parser
            // 层其实已经挡掉了,但这里仍按「丢弃」处理,免得将来放宽语法时
            // 凭空造出一个假的类型变量。
            TypeExpr::Bounded { name, bound, .. } => {
                if is_known_head(&name.to_ascii_uppercase()) {
                    return Ty::from_head(name, &[]);
                }
                Ty::Var {
                    name: name.clone(),
                    bound: Some(Box::new(Ty::from_type_expr(bound))),
                }
            }
        }
    }

    /// 统一的头名分派。`args` 为已映射好的类型参数(裸标识符为空)。
    fn from_head(name: &str, args: &[Ty]) -> Ty {
        let upper = name.to_ascii_uppercase();
        match upper.as_str() {
            // ---- 基类型 ----
            "INTEGER" | "INT" if args.is_empty() => Ty::Integer,
            "FLOAT" | "DOUBLE" if args.is_empty() => Ty::Float,
            "STRING" | "STR" if args.is_empty() => Ty::String,
            "BOOLEAN" | "BOOL" if args.is_empty() => Ty::Boolean,
            "NULL" | "NONE" if args.is_empty() => Ty::Null,

            // ---- 回退 / 顶底元 ----
            "DYNAMIC" | "ANY" if args.is_empty() => Ty::Dynamic,

            // ---- 容器与包装 ----
            // 注:`ARRAY` 裸写(无方括号)被 parser 特判拒绝
            // (`wlwl-parser/src/lib.rs:2407-2410`),所以下面这条
            // `args.is_empty()` 分支今天**不可从源码到达**,属防御性分支 ——
            // 保持映射全函数,不因将来 parser 放宽而 panic。
            "ARRAY" if args.is_empty() => Ty::Array(Box::new(Ty::Dynamic)),

            "ARRAY" if args.len() == 1 => Ty::Array(Box::new(args[0].clone())),
            "DICT" | "MAP" if args.is_empty() => {
                Ty::Dict(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic))
            }
            "DICT" | "MAP" if args.len() == 2 => {
                Ty::Dict(Box::new(args[0].clone()), Box::new(args[1].clone()))
            }
            "OPTION" | "MAYBE" if args.is_empty() => Ty::Option(Box::new(Ty::Dynamic)),
            "OPTION" | "MAYBE" if args.len() == 1 => Ty::Option(Box::new(args[0].clone())),
            "RESULT" if args.is_empty() => Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic)),
            "RESULT" if args.len() == 2 => {
                Ty::Result(Box::new(args[0].clone()), Box::new(args[1].clone()))
            }
            // `OK[T]` / `ERR[E]`(AST 注释点名的既有用法):
            // 未提及的那一侧保持 Dynamic,不做无依据的补全。
            "OK" if args.len() == 1 => Ty::Result(Box::new(args[0].clone()), Box::new(Ty::Dynamic)),
            "ERR" if args.len() == 1 => {
                Ty::Result(Box::new(Ty::Dynamic), Box::new(args[0].clone()))
            }

            // ---- 函数类型 ----
            // 暂定映射:参数表取全部类型参数,返回类型 Dynamic。**今天不可达**
            // (parser 不产 `FUN` 头),D-5 拍板后随语法一并定稿。
            "FUN" | "FUNCTION" => Ty::Fun {
                params: args.to_vec(),
                ret: Box::new(Ty::Dynamic),
            },

            // ---- 已知名但元数不对:保名保参,交给 A4 诊断 ----
            _ if is_known_head(&upper) => Ty::Named {
                name: name.to_string(),
                args: args.to_vec(),
            },

            // ---- 未知头 ----
            _ => Ty::Named {
                name: name.to_string(),
                args: args.to_vec(),
            },
        }
    }

    /// 两个类型的**最小公共上界**(join),用于 `IF` / `MATCH` 分支合流
    /// (ADR-0020 A2′)。
    ///
    /// 规则:
    /// - 相同即自身;
    /// - **任一侧为 `Dynamic` → `Dynamic`**。这条是刻意反向的:合流只能
    ///   比参与合流的类型更精确,绝不能更精确。某一支「不知道」时,结论
    ///   也只能是「不知道」—— 否则会把未标注分支的 `Dynamic` 悄悄收窄成
    ///   另一个分支的具体类型,凭空造出类型信息;
    /// - 否则取「谁可赋值给谁」的方向:能单向赋值就取宽的那个
    ///   (`Integer` 与 `Float` → `Float`,即 ADR-0010 的安全放宽方向);
    /// - 互不可赋值 → `Dynamic`(不猜)。
    pub fn lub(a: &Ty, b: &Ty) -> Ty {
        if a == b {
            return a.clone();
        }
        if a.is_dynamic() || b.is_dynamic() {
            return Ty::Dynamic;
        }
        // [v0.10 Step 9] 变量与具体类型合流 → 取具体那个(变量在调用点被
        // 实例化,分支合流后它就该是个确定类型了)。两个**不同**变量合流
        // 仍然是「不知道」—— 它们各自由各自的实参决定,这里无从知道是
        // 同一个,所以不猜(与「任一侧 Dynamic → Dynamic」同一条纪律)。
        match (a, b) {
            (Ty::Var { name: n1, .. }, Ty::Var { name: n2, .. }) => {
                return if n1 == n2 { a.clone() } else { Ty::Dynamic };
            }
            (Ty::Var { .. }, other) => return other.clone(),
            (other, Ty::Var { .. }) => return other.clone(),
            _ => {}
        }
        if a.is_assignable_to(b) {
            b.clone()
        } else if b.is_assignable_to(a) {
            a.clone()
        } else {
            Ty::Dynamic
        }
    }

    /// 是否为回退类型 `Dynamic`。
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Ty::Dynamic)
    }

    /// **注解边界判定**(ADR-0020 A4):实际类型能否填进声明类型。
    ///
    /// 与 [`Ty::is_assignable_to`] 是**两层不同的规则**,A4 才把它们分开:
    ///
    /// - `is_assignable_to` 是**类型关系** —— 结构一致 + 安全的数值放宽,
    ///   供类型层推理使用(谁能替代谁);
    /// - `satisfies_annotation` 是**注解边界** —— 在类型关系之上,额外承认
    ///   注解糖 `OPTION[T]`。
    ///
    /// `OPTION[T]` 是**纯注解侧**的可选包装(spec v0.9 §2.1 的 13 个运行时
    /// 类型里没有 `OPTION`;「无值」由 `NULL` 或 `RESULT` 表达)。它必须
    /// 有运行时见证,否则用户写 `LET(x: OPTION[INTEGER], v)` 时无从判断
    /// 什么算通过。故本方法承认两个见证:
    ///
    /// - `NULL` —— 「就是没有」;
    /// - `RESULT[T, ·]` —— 「或者是一个 T」,错误侧不要求已知。
    ///
    /// 反过来,**裸 `T` 不满足 `OPTION[T]`** —— 若它也通过,`OPTION` 就
    /// 退化成 `T` 的别名,失去全部意义。
    pub fn satisfies_annotation(&self, declared: &Ty) -> bool {
        if self.is_assignable_to(declared) {
            return true;
        }
        match declared {
            Ty::Option(inner) => match self {
                Ty::Null => true,
                Ty::Result(ok, _) => ok.is_assignable_to(inner),
                _ => false,
            },
            _ => false,
        }
    }

    /// `RESULT` 的成功侧载荷类型 —— `TRY` / `UNWRAP` / `UNWRAP_OR` /
    /// `OR_DIE` 的解包目标。
    ///
    /// **只在错误侧未知(`Dynamic`)时给出结论**:错误侧一旦具体化,`OK` /
    /// `ERR` 是**运行期分支**,静态层无法知道实际走了哪一支(那属于类型
    /// 收窄,A5 已推迟)。给不出结论就返回 `None` → 落 `Dynamic` → 不报,
    /// 符合「不误报优先」。
    pub fn unwrap_result(&self) -> Option<&Ty> {
        match self {
            Ty::Result(ok, err) if err.is_dynamic() => Some(ok),
            _ => None,
        }
    }

    /// 这个类型是否满足显式约束(Step 9 / D-5)。
    ///
    /// 本版**只有一个约束:`Comparable`**,而它的成员集合是**量出来的**,
    /// 不是猜的 —— 运行期 `<` / `>` / `<=` / `>=` 走 `wlwl_eval` 的
    /// `cmp_op`,它只放行**数值**(INTEGER / FLOAT)与 **STRING**,其余一律
    /// `E0030`。所以静态层放行的集合必须与它**逐项一致**,否则就会出现
    /// 「check 说行、run 抛错」或反过来的裂缝。
    ///
    /// 未知约束名(不是 `Comparable`)一律放行:约束集本版只有一条,
    /// 将来加了新约束再在这里扩;现在报「未知约束」等于凭空造一个诊断码
    /// 与一条用户无解的错误。
    pub fn satisfies_bound(&self, bound: &Ty) -> bool {
        let name = match bound {
            Ty::Named { name, args } if args.is_empty() => name.to_ascii_uppercase(),
            _ => return true,
        };
        match name.as_str() {
            "COMPARABLE" => {
                // `Dynamic` 是「不知道」—— 判不出来就不该拦(不误报优先)。
                if self.is_dynamic() {
                    return true;
                }
                matches!(self, Ty::Integer | Ty::Float | Ty::String)
            }
            _ => true,
        }
    }

    /// 本类型里出现过的类型变量(去重,按首次出现顺序)。
    ///
    /// 供实例化与签名渲染判断「这个类型里有没有变量」用 —— 绝大多数类型
    /// 一个变量都没有,这条让它们走不到实例化那套逻辑。
    pub fn vars(&self) -> Vec<&Ty> {
        let mut out: Vec<&Ty> = Vec::new();
        collect_vars(self, &mut out);
        out
    }

    /// 源类型能否赋给目标类型。
    ///
    /// 规则:
    /// - 任一侧为 [`Ty::Dynamic`] 即为真(顶/底元双向互通);
    /// - **`INTEGER -> FLOAT` 视为真**:这是 ADR-0010 明确写下的
    ///   *silent upcast, even under strict*。运行时与 `strict_types` 都
    ///   接受它,所以静态层**必须**接受 —— 否则 `gradual_typing = "error"`
    ///   会拒掉运行时认为合法的程序,直接违反「不误报」验收项;
    /// - 反方向 `FLOAT -> INTEGER` **不**视为真(收窄,本层拒绝);
    /// - 容器 / 包装逐位置递归;
    /// - 函数类型形参**逆变**、返回类型协变;
    /// - [`Ty::Named`] 要求同名、同元数、参数逐位置可赋值。
    pub fn is_assignable_to(&self, target: &Ty) -> bool {
        if self.is_dynamic() || target.is_dynamic() {
            return true;
        }
        match (self, target) {
            // 安全的数值放宽,与 ADR-0010 的运行时 transient cast 对齐。
            (Ty::Integer, Ty::Float) => true,
            (Ty::Integer, Ty::Integer)
            | (Ty::Float, Ty::Float)
            | (Ty::String, Ty::String)
            | (Ty::Boolean, Ty::Boolean)
            | (Ty::Null, Ty::Null) => true,

            (Ty::Array(a), Ty::Array(b)) => a.is_assignable_to(b),
            (Ty::Dict(k1, v1), Ty::Dict(k2, v2)) => {
                k1.is_assignable_to(k2) && v1.is_assignable_to(v2)
            }
            (Ty::Option(a), Ty::Option(b)) => a.is_assignable_to(b),
            (Ty::Result(t1, e1), Ty::Result(t2, e2)) => {
                t1.is_assignable_to(t2) && e1.is_assignable_to(e2)
            }
            (
                Ty::Fun {
                    params: p1,
                    ret: r1,
                },
                Ty::Fun {
                    params: p2,
                    ret: r2,
                },
            ) => {
                p1.len() == p2.len()
                    // 形参逆变:目标形参能喂给源形参才算安全。
                    && p1.iter().zip(p2.iter()).all(|(a, b)| b.is_assignable_to(a))
                    && r1.is_assignable_to(r2)
            }
            (Ty::Named { name: n1, args: a1 }, Ty::Named { name: n2, args: a2 }) => {
                n1 == n2
                    && a1.len() == a2.len()
                    && a1.iter().zip(a2.iter()).all(|(a, b)| a.is_assignable_to(b))
            }
            // [v0.10 Step 9] 类型变量。
            //
            // - 变量对任何具体类型:无约束就通吃;有约束就要满足约束
            //   (约束判定在 `satisfies_bound`);
            // - 两个变量之间**不看名字**:调用点还没实例化时,两个不同变量
            //   谁装谁都不知道 → 一律放行(不误报优先)。名字相同自然也通;
            // - 变量与非变量容器混合(`ARRAY[T]` 对 `ARRAY[INTEGER]`)由上面
            //   的容器分支递归下去,自然落到这两条上。
            // [v0.10 Step 9] 类型变量。规则不对称,是有意的:
            //
            // - 源侧是变量、目标侧是**具体**类型 → 放行。变量在调用点才
            //   实例化,这里它「还没定」;它自己要满足的约束是给**填进来
            //   的实参**看的,不是给它自己的。(写成 `target.satisfies_bound`
            //   之类的反向判定会误报。)
            // - 目标侧是变量、源侧是具体类型 → 源类型必须满足该变量的
            //   约束(`T: Comparable` 说的是「填进来的东西要可比」)。
            // - **两边都是变量** → 放行。调用点还没实例化,谁装谁都不知道,
            //   拿一个变量的未定类型去验另一个变量的约束只会误报。
            (Ty::Var { .. }, other) if !matches!(other, Ty::Var { .. }) => true,
            (_, Ty::Var { bound, .. }) if !matches!(self, Ty::Var { .. }) => match bound {
                None => true,
                Some(b) => self.satisfies_bound(b),
            },
            (Ty::Var { .. }, Ty::Var { .. }) => true,
            _ => false,
        }
    }
}

impl From<&TypeExpr> for Ty {
    fn from(expr: &TypeExpr) -> Ty {
        Ty::from_type_expr(expr)
    }
}

impl fmt::Display for Ty {
    /// 规范渲染,使用**方括号**(与 parser 产出一致,可往返)。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Integer => f.write_str("INTEGER"),
            Ty::Float => f.write_str("FLOAT"),
            Ty::String => f.write_str("STRING"),
            Ty::Boolean => f.write_str("BOOLEAN"),
            Ty::Null => f.write_str("NULL"),
            Ty::Dynamic => f.write_str("DYNAMIC"),
            Ty::Array(e) => write!(f, "ARRAY[{}]", e),
            Ty::Dict(k, v) => write!(f, "DICT[{}, {}]", k, v),
            Ty::Option(t) => write!(f, "OPTION[{}]", t),
            Ty::Result(t, e) => write!(f, "RESULT[{}, {}]", t, e),
            // [v0.10 Step 9] 类型变量渲染成源码可解析的 `T: Comparable` ——
            // Step 7 的签名生成器靠这一点把变量原样写进 `.wll.sig`。
            Ty::Var { name, bound } => match bound {
                None => f.write_str(name),
                Some(b) => write!(f, "{}: {}", name, b),
            },
            Ty::Fun { params, ret } => {
                let rendered: Vec<String> = params.iter().map(Ty::to_string).collect();
                write!(f, "FUN[{}] -> {}", rendered.join(", "), ret)
            }
            Ty::Named { name, args } if args.is_empty() => f.write_str(name),
            Ty::Named { name, args } => {
                let rendered: Vec<String> = args.iter().map(Ty::to_string).collect();
                write!(f, "{}[{}]", name, rendered.join(", "))
            }
        }
    }
}

/// 收集类型里的变量(Step 9)。
fn collect_vars<'a>(ty: &'a Ty, out: &mut Vec<&'a Ty>) {
    match ty {
        Ty::Var { .. } => {
            if !out.contains(&ty) {
                out.push(ty);
            }
        }
        Ty::Array(e) | Ty::Option(e) => collect_vars(e, out),
        Ty::Dict(k, v) | Ty::Result(k, v) => {
            collect_vars(k, out);
            collect_vars(v, out);
        }
        Ty::Fun { params, ret } => {
            for p in params {
                collect_vars(p, out);
            }
            collect_vars(ret, out);
        }
        Ty::Named { args, .. } => {
            for a in args {
                collect_vars(a, out);
            }
        }
        Ty::Integer | Ty::Float | Ty::String | Ty::Boolean | Ty::Null | Ty::Dynamic => {}
    }
}

/// **实例化**:把声明类型里的类型变量按实参类型绑定起来。
///
/// 返回实参是否**填得进**声明类型(含约束判定)。调用方在返回 `false` 时
/// 照常报既有的调用失配诊断 —— 报出来的类型是**未代入的声明类型**
/// (`T: Comparable`),所以消息里能看到约束,而不是被抹成实参类型。
///
/// 规则:
/// - 遇到变量:先验约束(不过 → `false`),再记绑定(同名再绑一次取 lub);
/// - 容器 / 包装 / `RESULT` 逐位置递归;
/// - 函数类型**不进变量**(形参/返回里的变量属于被调方自己的量化,本版
///   没有量化语法,进去只会把两个不同函数的变量混在一起);
/// - `Dynamic` 两侧一律通过,且**不产生绑定** —— 不知道的东西不许去
///   污染别的位置的变量。
pub fn instantiate(declared: &Ty, actual: &Ty, bindings: &mut Vec<(String, Ty)>) -> bool {
    match (declared, actual) {
        (Ty::Var { name, bound }, Ty::Var { .. }) => {
            // 两边都是变量:调用点还没实例化,谁装谁都不知道 → 放行,而且
            // **不记绑定**(拿一个未定类型去绑另一个只会把两边都拖成
            // 「不知道」)。
            let _ = (name, bound);
            true
        }
        (Ty::Var { name, bound }, _) => {
            if let Some(b) = bound {
                if !actual.satisfies_bound(b) {
                    return false;
                }
            }
            // `Dynamic` 是「不知道」,拿它去绑变量会把别的位置也拖成
            // 「不知道」—— 不绑,让它自然落回 `Dynamic`。
            if !actual.is_dynamic() {
                match bindings.iter_mut().find(|(n, _)| n == name) {
                    Some((_, prev)) => *prev = Ty::lub(prev, actual),
                    None => bindings.push((name.clone(), actual.clone())),
                }
            }
            true
        }
        (Ty::Array(a), Ty::Array(b)) => instantiate(a, b, bindings),
        (Ty::Dict(k1, v1), Ty::Dict(k2, v2)) => {
            let ok = instantiate(k1, k2, bindings);
            let ok2 = instantiate(v1, v2, bindings);
            ok && ok2
        }
        (Ty::Option(a), Ty::Option(b)) => instantiate(a, b, bindings),
        (Ty::Result(t1, e1), Ty::Result(t2, e2)) => {
            let ok = instantiate(t1, t2, bindings);
            let ok2 = instantiate(e1, e2, bindings);
            ok && ok2
        }
        (Ty::Named { name: n1, args: a1 }, Ty::Named { name: n2, args: a2 })
            if n1 == n2 && a1.len() == a2.len() =>
        {
            let mut ok = true;
            for (x, y) in a1.iter().zip(a2.iter()) {
                ok &= instantiate(x, y, bindings);
            }
            ok
        }
        _ => actual.satisfies_annotation(declared),
    }
}

/// 把类型里的变量按 `bindings` 代入;没绑定的变量**原地保留**(它还有
/// 约束要生效,不能变成 `Dynamic`)。
pub fn substitute(ty: &Ty, bindings: &[(String, Ty)]) -> Ty {
    match ty {
        Ty::Var { name, bound } => match bindings.iter().find(|(n, _)| n == name) {
            // 代入后仍要过一遍约束:绑定是逐位置记的,后来的绑定可能把它
            // 拉宽成不满足约束的类型(取 lub 的代价)。
            Some((_, t)) if bound.as_deref().is_none_or(|b| t.satisfies_bound(b)) => t.clone(),
            _ => ty.clone(),
        },
        Ty::Array(e) => Ty::Array(Box::new(substitute(e, bindings))),
        Ty::Dict(k, v) => Ty::Dict(
            Box::new(substitute(k, bindings)),
            Box::new(substitute(v, bindings)),
        ),
        Ty::Option(t) => Ty::Option(Box::new(substitute(t, bindings))),
        Ty::Result(t, e) => Ty::Result(
            Box::new(substitute(t, bindings)),
            Box::new(substitute(e, bindings)),
        ),
        Ty::Fun { params, ret } => Ty::Fun {
            params: params.iter().map(|p| substitute(p, bindings)).collect(),
            ret: Box::new(substitute(ret, bindings)),
        },
        Ty::Named { name, args } => Ty::Named {
            name: name.clone(),
            args: args.iter().map(|a| substitute(a, bindings)).collect(),
        },
        other => other.clone(),
    }
}

/// 头名是否在已知集合内(用于区分「元数错误」与「类型不认识」)。
///
/// 运行时真实类型(spec §2.1)里本层不结构化建模的 `TASK` / `CHANNEL` /
/// `CLASS` / `INSTANCE` 也在内 —— 它们走 `Named` 是**有意的**,不是不认识。
fn is_known_head(upper: &str) -> bool {
    matches!(
        upper,
        "TASK"
            | "CHANNEL"
            | "CLASS"
            | "INSTANCE"
            | "ARRAY"
            | "DICT"
            | "MAP"
            | "OPTION"
            | "MAYBE"
            | "RESULT"
            | "OK"
            | "ERR"
            | "FUN"
            | "FUNCTION"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_ast::Expr;
    use wlwl_parser::parse;

    /// 经**真实 parser** 走一圈再映射 —— 这样锁的是「源码 → IR」的实际
    /// 通路,而不是手工构造的 `TypeExpr`。
    ///
    /// 包装成 `FUN((x: <annotation>), x);` 是因为 `parse_type_annotation`
    /// 只在形参 / 返回值位置可达(它以顶层 `,` / `)` 收尾)。
    fn ty_of_annotation(annotation: &str) -> Ty {
        let src = format!("FUN((x: {annotation}), x);");
        let expr = parse(&src, "ty.wll").expect("annotation must parse");
        let Expr::Fun { params, .. } = expr else {
            panic!("expected a FUN expression");
        };
        let ann = params[0]
            .type_annotation
            .as_ref()
            .expect("param carries an annotation");
        Ty::from_type_expr(&ann.expr)
    }

    /// 返回值注解的映射(参数表收尾后紧跟 `: Type`,再跟 `, body`)。
    fn ty_of_return_annotation(annotation: &str) -> Ty {
        let src = format!("FUN((x): {annotation}, x);");
        let expr = parse(&src, "ty.wll").expect("return annotation must parse");
        let Expr::Fun { return_type, .. } = expr else {
            panic!("expected a FUN expression");
        };
        let ann = return_type.expect("return type annotation present");
        Ty::from_type_expr(&ann.expr)
    }

    /// 直接构造 `TypeExpr::Ident` —— 用于锁「parser 够不到」的防御性分支。
    ///
    /// parser 对 `ARRAY` 特判、强制要求方括号
    /// (`wlwl-parser/src/lib.rs:2407-2410`:命中 `ARRAY` 就直接进
    /// `parse_braced`),所以**裸 `ARRAY` 注解在 v0.9 源码里写不出来**;
    /// 其余头(`DICT` / `OPTION` / `RESULT`)走通用路径,裸写能解析成 `Ident`。
    fn ty_of_bare_ident(name: &str) -> Ty {
        Ty::from_type_expr(&TypeExpr::Ident {
            name: name.to_string(),
            span: wlwl_ast::Span::new("ty.wll", 1, 1),
        })
    }

    // ---- 锁测试 1:TypeExpr → IR 映射与往返 ----

    #[test]
    fn type_ir_roundtrip() {
        let cases: &[(&str, Ty)] = &[
            ("INTEGER", Ty::Integer),
            ("FLOAT", Ty::Float),
            ("STRING", Ty::String),
            ("BOOLEAN", Ty::Boolean),
            ("NULL", Ty::Null),
            ("ARRAY[INTEGER]", Ty::Array(Box::new(Ty::Integer))),
            (
                "DICT[STRING, INTEGER]",
                Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)),
            ),
            ("OPTION[STRING]", Ty::Option(Box::new(Ty::String))),
            (
                "RESULT[INTEGER, STRING]",
                Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String)),
            ),
            (
                "OK[INTEGER]",
                Ty::Result(Box::new(Ty::Integer), Box::new(Ty::Dynamic)),
            ),
            (
                "ERR[STRING]",
                Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::String)),
            ),
            (
                "ARRAY[ARRAY[INTEGER]]",
                Ty::Array(Box::new(Ty::Array(Box::new(Ty::Integer)))),
            ),
            ("int", Ty::Integer),
            ("String", Ty::String),
        ];

        for (source, expected) in cases {
            assert_eq!(&ty_of_annotation(source), expected, "source: {source}");
        }
    }

    /// 渲染回源码必须能被 parser 读回同一个类型 —— 这是选方括号记法
    /// 而不是尖括号的**唯一理由**,必须锁住。
    #[test]
    fn display_roundtrips_through_parser() {
        let tys = [
            Ty::Integer,
            Ty::Array(Box::new(Ty::Integer)),
            Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)),
            Ty::Option(Box::new(Ty::String)),
            Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String)),
            Ty::Array(Box::new(Ty::Dict(
                Box::new(Ty::String),
                Box::new(Ty::Array(Box::new(Ty::Float))),
            ))),
        ];
        for ty in tys {
            let rendered = ty.to_string();
            assert_eq!(ty_of_annotation(&rendered), ty, "rendered: {rendered}");
        }
    }

    #[test]
    fn return_annotation_maps_through_same_path() {
        assert_eq!(ty_of_return_annotation("INTEGER"), Ty::Integer);
        assert_eq!(
            ty_of_return_annotation("ARRAY[STRING]"),
            Ty::Array(Box::new(Ty::String))
        );
    }

    #[test]
    fn named_types_keep_name_and_args() {
        assert_eq!(
            ty_of_annotation("MY_TYPE"),
            Ty::Named {
                name: "MY_TYPE".into(),
                args: vec![]
            }
        );
        // 运行时真实类型(spec §2.1)但本层不结构化建模 -> 有意走 Named。
        assert_eq!(
            ty_of_annotation("CHANNEL"),
            Ty::Named {
                name: "CHANNEL".into(),
                args: vec![]
            }
        );
        // 识别得出但元数不对:保名保参,不静默丢弃。
        assert_eq!(
            ty_of_annotation("DICT[INTEGER]"),
            Ty::Named {
                name: "DICT".into(),
                args: vec![Ty::Integer]
            }
        );
    }

    // ---- 锁测试 2:Dynamic 回退 ----

    #[test]
    fn dynamic_fallback() {
        // 裸容器头:形状已知、元素未知 -> 形状 + Dynamic 参数。
        // `DICT` / `OPTION` / `RESULT` 裸写能过 parser,走真实通路。
        assert_eq!(
            ty_of_annotation("DICT"),
            Ty::Dict(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic))
        );
        assert_eq!(
            ty_of_annotation("RESULT"),
            Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic))
        );
        assert_eq!(
            ty_of_annotation("OPTION"),
            Ty::Option(Box::new(Ty::Dynamic))
        );
        assert_eq!(ty_of_annotation("ANY"), Ty::Dynamic);
        assert_eq!(ty_of_annotation("DYNAMIC"), Ty::Dynamic);
        // 裸 `ARRAY` 被 parser 拒(见下),只能直接构造 Ident 验证
        // 映射的防御性分支同样给出「形状 + Dynamic」。
        assert_eq!(ty_of_bare_ident("ARRAY"), Ty::Array(Box::new(Ty::Dynamic)));

        // 顶/底元:双向互通。
        assert!(Ty::Dynamic.is_assignable_to(&Ty::Integer));
        assert!(Ty::Integer.is_assignable_to(&Ty::Dynamic));
        assert!(Ty::Dynamic.is_dynamic());
        assert!(!Ty::Integer.is_dynamic());
        // Dynamic 可穿过容器参数。
        assert!(
            Ty::Array(Box::new(Ty::Dynamic)).is_assignable_to(&Ty::Array(Box::new(Ty::Integer)))
        );
        // 未标注处的回退类型就是 Dynamic。
        assert!(Ty::Dynamic.is_assignable_to(&Ty::Array(Box::new(Ty::String))));
    }

    /// 锁住「裸 `ARRAY` 注解在 v0.9 语法里写不出来」这一事实。
    ///
    /// 该特判是 parser 的既有行为(`wlwl-parser/src/lib.rs:2407-2410`),
    /// 不是本 crate 的选择。spec v0.10 补写 `Type` 产生式时需要知道:
    /// 容器头的类型参数在 v0.9 语法里**强制**出现。
    #[test]
    fn bare_array_annotation_is_rejected_by_the_parser() {
        let err = parse("FUN((x: ARRAY), x);", "ty.wll").expect_err("bare ARRAY is rejected");
        assert!(
            err.to_string().contains("expected `[` after `ARRAY`"),
            "unexpected diagnostic: {err}"
        );
    }

    #[test]
    fn assignability_allows_only_the_safe_numeric_widening() {
        assert!(Ty::Integer.is_assignable_to(&Ty::Integer));
        // ADR-0010: INTEGER -> FLOAT 是 silent upcast,even under strict。
        // 静态层必须一致,否则 error 模式会拒掉运行时合法的程序。
        assert!(Ty::Integer.is_assignable_to(&Ty::Float));
        // 收窄方向仍然拒绝。
        assert!(!Ty::Float.is_assignable_to(&Ty::Integer));
        assert!(
            Ty::Array(Box::new(Ty::Integer)).is_assignable_to(&Ty::Array(Box::new(Ty::Integer)))
        );
        assert!(Ty::Array(Box::new(Ty::Integer)).is_assignable_to(&Ty::Array(Box::new(Ty::Float))));
        assert!(
            !Ty::Array(Box::new(Ty::Integer)).is_assignable_to(&Ty::Array(Box::new(Ty::String)))
        );
        assert!(!Ty::Integer.is_assignable_to(&Ty::Array(Box::new(Ty::Integer))));
    }

    #[test]
    fn fun_type_is_ir_only_and_never_reachable_from_parser() {
        // parser 产不出 `FUN` 头(wlwl-ast 标注 reserved for v0.4),
        // 因此本变体只能直接构造;锁住「IR 存在但语法未接」这一事实。
        let ty = ty_of_annotation("FUN[INTEGER, STRING]");
        assert_eq!(
            ty,
            Ty::Fun {
                params: vec![Ty::Integer, Ty::String],
                ret: Box::new(Ty::Dynamic)
            }
        );
        assert_eq!(ty.to_string(), "FUN[INTEGER, STRING] -> DYNAMIC");
        assert!(!ty.is_assignable_to(&Ty::Integer));
        assert!(Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::String)
        }
        .is_assignable_to(&Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::String)
        }));
    }

    // ---- v0.10 Step 9 (P1-2 / D-5): 类型变量与显式约束 ----

    fn var(name: &str) -> Ty {
        Ty::Var {
            name: name.to_string(),
            bound: None,
        }
    }

    fn bounded(name: &str, bound: &str) -> Ty {
        Ty::Var {
            name: name.to_string(),
            bound: Some(Box::new(Ty::Named {
                name: bound.to_string(),
                args: Vec::new(),
            })),
        }
    }

    /// `Comparable` 的成员集合是**从运行期量出来的**:`cmp_op` 只放行
    /// 数值与 STRING,其余 `E0030`。这条测试就是那条测量的静态侧对照 ——
    /// 静态层多放一个或少放一个都会在这里破。
    #[test]
    fn comparable_is_exactly_the_runtime_comparison_domain() {
        let c = Ty::Named {
            name: "Comparable".into(),
            args: Vec::new(),
        };
        for ok in [Ty::Integer, Ty::Float, Ty::String] {
            assert!(ok.satisfies_bound(&c), "{ok} must be Comparable");
        }
        for no in [
            Ty::Boolean,
            Ty::Null,
            Ty::Array(Box::new(Ty::Integer)),
            Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)),
            Ty::Fun {
                params: vec![],
                ret: Box::new(Ty::Integer),
            },
        ] {
            assert!(!no.satisfies_bound(&c), "{no} must NOT be Comparable");
        }
        // 未知的东西永远放行(不误报优先):`Dynamic` 判不了就不该拦。
        assert!(Ty::Dynamic.satisfies_bound(&c));
        // 未知约束名也放行 —— 约束集本版只有一条。
        assert!(
            Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)).satisfies_bound(&Ty::Named {
                name: "NoSuchBound".into(),
                args: vec![]
            })
        );
    }

    #[test]
    fn an_unconstrained_variable_accepts_anything_and_a_bounded_one_does_not() {
        assert!(Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)).is_assignable_to(&var("T")));
        assert!(!Ty::Boolean.is_assignable_to(&bounded("T", "Comparable")));
        assert!(Ty::Integer.is_assignable_to(&bounded("T", "Comparable")));
        // 两个变量之间不看名字:调用点还没实例化时谁装谁都不知道。
        assert!(var("T").is_assignable_to(&var("U")));
        assert!(bounded("T", "Comparable").is_assignable_to(&var("U")));
    }

    /// 容器里的变量逐位置生效 —— 这正是计划书说的「`ARRAY[INTEGER]` 与
    /// `ARRAY[STRING]` 误用」那一类。
    #[test]
    fn a_bounded_variable_inside_a_container_is_checked_positionally() {
        let declared = Ty::Array(Box::new(bounded("T", "Comparable")));
        assert!(Ty::Array(Box::new(Ty::Integer)).is_assignable_to(&declared));
        assert!(Ty::Array(Box::new(Ty::String)).is_assignable_to(&declared));
        assert!(!Ty::Array(Box::new(Ty::Boolean)).is_assignable_to(&declared));
        assert!(declared.vars().len() == 1);
        assert!(Ty::Integer.vars().is_empty());
    }

    /// 实例化:把变量绑到实参上,并把绑定带进返回类型。
    #[test]
    fn instantiate_binds_variables_and_enforces_the_bound() {
        let declared = Ty::Fun {
            params: vec![bounded("T", "Comparable")],
            ret: Box::new(var("T")),
        };
        let (Ty::Fun { params, ret }, _) = (&declared, ()) else {
            unreachable!()
        };

        let mut ok_bindings = Vec::new();
        assert!(instantiate(&params[0], &Ty::String, &mut ok_bindings));
        assert_eq!(ok_bindings, vec![("T".to_string(), Ty::String)]);
        // 返回类型里的同一个变量被代入成实参类型 —— 这就是「泛型」的
        // 精度收益(无泛型的语言只能落 `DYNAMIC`)。
        assert_eq!(
            substitute(ret, &ok_bindings),
            Ty::String,
            "the return type must be instantiated with the argument type"
        );

        // 违反约束 → 实例化失败,由调用点报既有的调用失配。
        let mut bad = Vec::new();
        assert!(!instantiate(&params[0], &Ty::Boolean, &mut bad));
        assert!(bad.is_empty(), "a rejected binding must not be recorded");
    }

    #[test]
    fn dynamic_never_becomes_a_variable_binding() {
        // 「不知道」不许污染别的位置的变量:不绑 → 代入时保留变量原样,
        // 自然落回「不知道」。
        let mut bindings = Vec::new();
        assert!(instantiate(&var("T"), &Ty::Dynamic, &mut bindings));
        assert!(bindings.is_empty());
        assert_eq!(substitute(&var("T"), &bindings), var("T"));
    }

    #[test]
    fn a_variable_meets_a_concrete_type_in_a_lub() {
        // 分支合流:变量在调用点被实例化后,合流就该是个确定类型。
        assert_eq!(Ty::lub(&var("T"), &Ty::Integer), Ty::Integer);
        assert_eq!(Ty::lub(&Ty::Integer, &var("T")), Ty::Integer);
        // 仍然是「不知道」的照旧是「不知道」。
        assert_eq!(Ty::lub(&var("T"), &var("U")), Ty::Dynamic);
        assert_eq!(Ty::lub(&var("T"), &Ty::Dynamic), Ty::Dynamic);
    }

    /// 变量的 `Display` 必须是**源码能解析回去**的形状 —— Step 7 的
    /// 签名生成器靠它把泛型写进 `.wll.sig`。
    #[test]
    fn a_bounded_variable_renders_back_into_source_syntax() {
        assert_eq!(bounded("T", "Comparable").to_string(), "T: Comparable");
        assert_eq!(var("T").to_string(), "T");
        assert_eq!(
            Ty::Array(Box::new(bounded("T", "Comparable"))).to_string(),
            "ARRAY[T: Comparable]"
        );
    }
}
