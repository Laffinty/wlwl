/*
 * wlwl_ext.h -- WLWL 宿主侧原生动态库扩展 ABI v1
 * ============================================================================
 *
 * 扩展作者实现这个头文件、导出一个符号 `wlwl_ext_v1`,宿主就能在**显式
 * 授权**下(`wlwl --ext <path>`)加载它,把它声明的成员挂成可 `IMPORT` 的
 * 命名空间。
 *
 * ⚠⚠⚠ **加载一个原生库 = 把整个进程的权限交给它。**
 *   它不是沙箱:它能读文件、连网络、改内存、起进程。宿主在加载前会打印
 *   一行醒目告警 —— 那句话是事实陈述,不是免责声明。
 *
 * ⚠ **语言面绝不提供任何 LOAD 成员** ⇒ 脚本**不能**触发代码加载。
 *   只有 CLI `--ext` / 环境变量 `WLWL_EXT_PATH` 这两条**显式**通道能加载;
 *   宿主**绝不**从项目配置 / 项目目录 / `./ext/` 自动发现扩展。
 *
 * ---------------------------------------------------------------------------
 * 调用约定
 * ---------------------------------------------------------------------------
 *
 * 成员调用:
 *
 *   args_json  (入)  UTF-8 JSON 数组文本,**由宿主分配、只读**。
 *                       扩展**不得**释放它(调用返回后宿主会释放)。
 *   out_json   (出)  扩展把结果写进 `*out_json`:
 *                       - 成功:`{"value": <任意 JSON>}`
 *                       - 失败:`{"err": {"kind": "...", "op": "...", "reason": "..."}}`
 *                     ⚠ **那块内存必须用 `ctx->host->alloc()` 分配** ——
 *                       宿主会用 `ctx->host->free()` 释放它。
 *                       ⚠ Windows 上这是**必须**的:各用各的分配器
 *                       (ucrt vs msvcrt)⇒ 跨堆释放 ⇒ 崩。
 *   返回值     0 = 成功;非 0 = 失败(失败应同时给出 `{"err": ...}` 载荷)。
 *
 * ⚠ `err.kind` 是**开放字符串**(不是原生诊断码)⇒ 宿主把它变成 wlwl 的
 *   `ERR` 值,不是诊断。
 *
 * ---------------------------------------------------------------------------
 * 元数
 * ---------------------------------------------------------------------------
 *
 * `arity_min` / `arity_max` 由**宿主先校验**(不合规就不调扩展)⇒
 * 宿主报既有诊断码 `E0022`(元数错)。`arity_max == 0xFFFFFFFF` = 变参。
 *
 * ---------------------------------------------------------------------------
 * 命名空间
 * ---------------------------------------------------------------------------
 *
 * `path` 是命名空间路径(如 `"tls"`):
 *   - 不得是 `wlwl:std.*`(那是标准库的);
 *   - 不得占用任何既有命名空间(std 的短名、项目清单里的命名空间);
 *   - 不得含 `:`(运行期派发键是 `<ns>:<member>`)。
 *
 * ⚠ 扩展命名空间**不进附录 A、不进公共面清单** ⇒ 它不是规范的一部分。
 *
 * ---------------------------------------------------------------------------
 * 示例(一个最小扩展)
 * ---------------------------------------------------------------------------
 *
 *   #include "wlwl_ext.h"
 *   #include <string.h>
 *   #include <stdlib.h>
 *
 *   static int32_t WLWL_EXT_CALL hello_call(wlwl_ext_ctx_t* ctx,
 *                                            const char* args_json,
 *                                            char** out_json) {
 *       const char* body = "{\"value\": {\"greeting\": \"hi\"}}";
 *       size_t n = strlen(body) + 1;
 *       char* buf = (char*)ctx->host->alloc((uint32_t)n);
 *       if (!buf) return 1;
 *       memcpy(buf, body, n);
 *       *out_json = buf;
 *       (void)args_json;
 *       return 0;
 *   }
 *
 *   static const wlwl_ext_member_t MEMBERS[] = {
 *       { "hello", 0, 0xFFFFFFFF, hello_call },
 *   };
 *   static const wlwl_ext_ns_t NAMESPACES[] = {
 *       { "demo", 1, MEMBERS },
 *   };
 *
 *   WLWL_EXT_V1_EXPORT {
 *       .abi_version   = 1,
 *       .struct_size   = (uint32_t)sizeof(wlwl_ext_v1_t),
 *       .host_version  = "demo 1.0",
 *       .ns_count      = 1,
 *       .namespaces    = NAMESPACES,
 *       .host          = NULL,   /* 由宿主填写 */
 *   };
 *
 *   // 用法:IMPORT("demo", ["hello"]); hello();
 *
 * ---------------------------------------------------------------------------
 * 导出符号
 * ---------------------------------------------------------------------------
 *
 *   #if defined(_WIN32)
 *   #  define WLWL_EXT_V1_EXPORT __declspec(dllexport)
 *   #else
 *   #  define WLWL_EXT_V1_EXPORT __attribute__((visibility("default")))
 *   #endif
 */

#ifndef WLWL_EXT_H
#define WLWL_EXT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * 调用约定:Win32 用 __stdcall,其它平台用平台默认(cdecl)。
 * ⚠ 必须与 Rust 侧的 `extern "system"` 一致 —— 不一致 ⇒ 栈错乱。
 */
#if defined(_WIN32)
#  define WLWL_EXT_CALL __stdcall
#else
#  define WLWL_EXT_CALL
#endif

/* 导出符号的属性。 */
#if defined(_WIN32)
#  define WLWL_EXT_V1_EXPORT __declspec(dllexport)
#elif defined(__GNUC__)
#  define WLWL_EXT_V1_EXPORT __attribute__((visibility("default")))
#else
#  define WLWL_EXT_V1_EXPORT
#endif

/* 扩展必须导出的符号名。 */
#define WLWL_EXT_SYMBOL_NAME "wlwl_ext_v1"

/* ABI 版本:宿主只接受 1。 */
#define WLWL_EXT_ABI_VERSION 1u

/* `arity_max` 的哨兵:变参。 */
#define WLWL_EXT_ARITY_UNBOUNDED 0xFFFFFFFFu

typedef struct wlwl_ext_ctx wlwl_ext_ctx_t;
typedef struct wlwl_ext_host wlwl_ext_host_t;

/* 成员实现。 */
typedef int32_t (WLWL_EXT_CALL *wlwl_ext_call_fn)(wlwl_ext_ctx_t* ctx,
                                                   const char* args_json,
                                                   char** out_json);

/* 宿主分配器:扩展**必须**用它分配交回来的字符串。 */
typedef void* (WLWL_EXT_CALL *wlwl_ext_alloc_fn)(uint32_t size);

/* 宿主释放器:宿主用它释放扩展交回来的字符串(同一个分配器)。 */
typedef void (WLWL_EXT_CALL *wlwl_ext_free_fn)(void* ptr);

/* 扩展向宿主报告一条信息。 */
typedef void (WLWL_EXT_CALL *wlwl_ext_report_fn)(wlwl_ext_ctx_t* ctx,
                                                  const char* kind,
                                                  const char* reason);

/*
 * 询问句柄是否就绪。
 * ⚠ ABI v1 **不支持异步扩展成员** ⇒ 宿主恒返回 -1。
 */
typedef int32_t (WLWL_EXT_CALL *wlwl_ext_await_ready_fn)(wlwl_ext_ctx_t* ctx,
                                                          uint32_t handle,
                                                          uint32_t timeout_ms);

/* 一个扩展成员。 */
struct wlwl_ext_member {
    const char* name;      /* 成员名(UTF-8,NUL 结尾);不得为空。 */
    uint32_t arity_min;    /* 最少实参个数。 */
    uint32_t arity_max;    /* 最多实参个数;0xFFFFFFFF = 变参。 */
    wlwl_ext_call_fn call; /* 成员实现。 */
};

/* 一个扩展命名空间。 */
struct wlwl_ext_ns {
    const char* path;                          /* 命名空间路径,如 "tls"。 */
    uint32_t member_count;                     /* 成员数。 */
    const struct wlwl_ext_member* members;     /* 成员表。 */
};

/* 宿主 → 扩展 的回调表(由宿主填写,扩展只读)。 */
struct wlwl_ext_host {
    wlwl_ext_alloc_fn alloc;         /* 分配器(跨 CRT 的关键)。 */
    wlwl_ext_free_fn free;           /* 释放器(与 alloc 同一个)。 */
    wlwl_ext_report_fn report;       /* 报告通道。 */
    wlwl_ext_await_ready_fn await_ready; /* v1 恒不支持。 */
};

/* 每次成员调用时交给扩展的上下文。 */
struct wlwl_ext_ctx {
    const struct wlwl_ext_host* host; /* 宿主回调表。 */
    const char* member;               /* 当前被调用的成员名。 */
};

/* 扩展导出的**唯一**符号指向的结构。 */
struct wlwl_ext_v1 {
    uint32_t abi_version;  /* 必须 == WLWL_EXT_ABI_VERSION。 */
    uint32_t struct_size;  /* sizeof(struct wlwl_ext_v1)。 */
    const char* host_version; /* 宿主版本文本(可为 NULL)。 */
    uint32_t ns_count;      /* 命名空间个数。 */
    const struct wlwl_ext_ns* namespaces; /* 命名空间表。 */
    struct wlwl_ext_host* host; /* 由宿主填写。 */
};

/* 兼容别名:题面里的类型名。 */
typedef struct wlwl_ext_member wlwl_ext_member_t;
typedef struct wlwl_ext_ns wlwl_ext_ns_t;
typedef struct wlwl_ext_v1 wlwl_ext_v1_t;

#ifdef __cplusplus
}
#endif

#endif /* WLWL_EXT_H */
