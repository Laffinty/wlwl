//! 静态类型环境(ADR-0020 A1)。
//!
//! 词法作用域链:每次进入块 / 循环 / 形参表 push 一层,离开 pop 一层。
//! 查询自内向外,先命中者胜 —— 与 spec v0.9 §3.2 的「内层同名遮蔽外层」
//! 一致。
//!
//! 本层**不做**类型收窄(流敏感分析推迟,ADR-0020 明确砍掉 A5),也**不做**
//! let-多态:同一个名字重新绑定就是覆盖,与 §3.2 的「重绑定」语义相同。

use std::collections::HashMap;

use crate::ty::Ty;

/// 名字 → 静态类型的词法作用域链。
#[derive(Debug, Clone)]
pub struct TypeEnv {
    /// `scopes[0]` 是根作用域(模块顶层),**永不弹出**。
    scopes: Vec<HashMap<String, Ty>>,
}

impl Default for TypeEnv {
    /// 必须走 `new()` 而不是 `derive`:`derive` 会给 `Vec::new()` ——
    /// 那样根作用域缺失,`bind` 静默 no-op、`lookup` 恒为 `None`。
    fn default() -> TypeEnv {
        TypeEnv::new()
    }
}

impl TypeEnv {
    /// 新建环境,只含根作用域。
    pub fn new() -> TypeEnv {
        TypeEnv {
            scopes: vec![HashMap::new()],
        }
    }

    /// 进入一层新作用域。
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// 离开最内层作用域。
    ///
    /// 根作用域**不弹出** —— 对根调用是 no-op,不做 panic:作用域配对是
    /// 调用方的责任,而类型环境只是静态检查的辅助结构,不该成为运行期
    /// 崩溃的来源。
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// 在**当前**作用域绑定一个名字。重复绑定即覆盖(重绑定)。
    pub fn bind(&mut self, name: impl Into<String>, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), ty);
        }
    }

    /// 自内向外查询,返回最先命中的绑定。
    pub fn lookup(&self, name: &str) -> Option<&Ty> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// 名字是否在任意作用域可见。
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// 当前作用域层数。恒 `>= 1`。
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_walks_innermost_first() {
        let mut env = TypeEnv::new();
        env.bind("x", Ty::Integer);
        env.push_scope();
        env.bind("x", Ty::String);
        assert_eq!(env.lookup("x"), Some(&Ty::String));
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&Ty::Integer));
        assert!(env.contains("x"));
        assert!(!env.contains("y"));
    }

    #[test]
    fn rebind_within_one_scope_overwrites() {
        let mut env = TypeEnv::new();
        env.bind("x", Ty::Integer);
        env.bind("x", Ty::Float);
        assert_eq!(env.lookup("x"), Some(&Ty::Float));
    }

    #[test]
    fn root_scope_is_never_popped() {
        let mut env = TypeEnv::new();
        env.bind("x", Ty::Integer);
        for _ in 0..3 {
            env.pop_scope();
        }
        assert_eq!(env.depth(), 1);
        assert_eq!(env.lookup("x"), Some(&Ty::Integer));
    }

    #[test]
    fn depth_tracks_nesting() {
        let mut env = TypeEnv::new();
        assert_eq!(env.depth(), 1);
        env.push_scope();
        env.push_scope();
        assert_eq!(env.depth(), 3);
        env.pop_scope();
        assert_eq!(env.depth(), 2);
    }

    #[test]
    fn binding_in_popped_scope_is_gone() {
        let mut env = TypeEnv::new();
        env.push_scope();
        env.bind("local", Ty::Boolean);
        assert!(env.contains("local"));
        env.pop_scope();
        assert!(!env.contains("local"));
    }

    #[test]
    fn default_is_a_usable_single_root_env() {
        let mut env = TypeEnv::default();
        assert_eq!(env.depth(), 1);
        env.bind("x", Ty::Null);
        assert_eq!(env.lookup("x"), Some(&Ty::Null));
    }
}
