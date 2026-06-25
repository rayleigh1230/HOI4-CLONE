// WASM 封装层: 加载引擎 + 内存读写(沿用旧 index.html 的 FFI 契约)
let wasm = null;

export async function loadWasm() {
  const resp = await fetch('hoi4_clone.wasm?v=' + Date.now());
  const bytes = await resp.arrayBuffer();
  const result = await WebAssembly.instantiate(bytes, { env: {} });
  wasm = result.instance;
  return wasm;
}

export function getWasm() { return wasm; }

// 把 Rust 返回的 null-terminated C 字符串读成 JS string
export function readCString(ptr) {
  const mem = new Uint8Array(wasm.exports.memory.buffer);
  let end = ptr;
  while (mem[end] !== 0) end++;
  return new TextDecoder('utf-8').decode(mem.subarray(ptr, end));
}

// 传 JS string 给 Rust: 编码后写入 wasm 内存, 返回 {ptr, len}
export function passStr(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = wasm.exports.engine_alloc(bytes.length);
  new Uint8Array(wasm.exports.memory.buffer).set(bytes, ptr);
  return { ptr, len: bytes.length };
}
