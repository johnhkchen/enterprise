// SPDX-License-Identifier: AGPL-3.0-or-later  ·  Copyright (c) 2026 John Chen
// Minimal, dependency-free NBT (Java edition, big-endian) reader/writer on node's zlib.
//
// Values are TYPED trees, so anything we read can be written back byte-faithfully (sign text, chest contents,
// entities survive a round trip untouched):
//   { type: "compound", value: { name: tag, ... } }   { type: "list", value: { type: "int", value: [1,2,3] } }
//   { type: "int", value: 5 }  { type: "long", value: 5n }  { type: "string", value: "x" } ...
// `simplify(tag)` turns a typed tree into plain JS for reading convenience; `t.*` builds typed tags.
import { gunzipSync, gzipSync, inflateSync } from "node:zlib";

const TYPES = ["end", "byte", "short", "int", "long", "float", "double", "byteArray", "string", "list", "compound",
  "intArray", "longArray"];
const ID = Object.fromEntries(TYPES.map((n, i) => [n, i]));

class Reader {
  constructor(buf) { this.b = buf; this.o = 0; }
  u8() { return this.b.readUInt8(this.o++); }
  i8() { return this.b.readInt8(this.o++); }
  i16() { const v = this.b.readInt16BE(this.o); this.o += 2; return v; }
  u16() { const v = this.b.readUInt16BE(this.o); this.o += 2; return v; }
  i32() { const v = this.b.readInt32BE(this.o); this.o += 4; return v; }
  i64() { const v = this.b.readBigInt64BE(this.o); this.o += 8; return v; }
  f32() { const v = this.b.readFloatBE(this.o); this.o += 4; return v; }
  f64() { const v = this.b.readDoubleBE(this.o); this.o += 8; return v; }
  str() { const n = this.u16(); const s = this.b.toString("utf8", this.o, this.o + n); this.o += n; return s; }
  payload(type) {
    switch (type) {
      case "byte": return this.i8();
      case "short": return this.i16();
      case "int": return this.i32();
      case "long": return this.i64();
      case "float": return this.f32();
      case "double": return this.f64();
      case "string": return this.str();
      case "byteArray": { const n = this.i32(); const a = []; for (let i = 0; i < n; i++) a.push(this.i8()); return a; }
      case "intArray": { const n = this.i32(); const a = []; for (let i = 0; i < n; i++) a.push(this.i32()); return a; }
      case "longArray": { const n = this.i32(); const a = []; for (let i = 0; i < n; i++) a.push(this.i64()); return a; }
      case "list": {
        const et = TYPES[this.u8()]; const n = this.i32(); const a = [];
        for (let i = 0; i < n; i++) a.push(this.payload(et));
        return { type: et, value: a };
      }
      case "compound": {
        const out = {};
        for (;;) {
          const t = TYPES[this.u8()];
          if (t === "end") return out;
          const name = this.str();
          out[name] = { type: t, value: this.payload(t) };
        }
      }
      default: throw new Error(`nbt: unknown tag type ${type}`);
    }
  }
}

class Writer {
  constructor() { this.parts = []; }
  push(b) { this.parts.push(b); }
  u8(v) { const b = Buffer.alloc(1); b.writeUInt8(v); this.push(b); }
  i8(v) { const b = Buffer.alloc(1); b.writeInt8(v); this.push(b); }
  i16(v) { const b = Buffer.alloc(2); b.writeInt16BE(v); this.push(b); }
  u16(v) { const b = Buffer.alloc(2); b.writeUInt16BE(v); this.push(b); }
  i32(v) { const b = Buffer.alloc(4); b.writeInt32BE(v); this.push(b); }
  i64(v) { const b = Buffer.alloc(8); b.writeBigInt64BE(BigInt(v)); this.push(b); }
  f32(v) { const b = Buffer.alloc(4); b.writeFloatBE(v); this.push(b); }
  f64(v) { const b = Buffer.alloc(8); b.writeDoubleBE(v); this.push(b); }
  str(s) { const b = Buffer.from(s, "utf8"); this.u16(b.length); this.push(b); }
  payload(type, v) {
    switch (type) {
      case "byte": return this.i8(v);
      case "short": return this.i16(v);
      case "int": return this.i32(v);
      case "long": return this.i64(v);
      case "float": return this.f32(v);
      case "double": return this.f64(v);
      case "string": return this.str(v);
      case "byteArray": this.i32(v.length); for (const x of v) this.i8(x); return;
      case "intArray": this.i32(v.length); for (const x of v) this.i32(x); return;
      case "longArray": this.i32(v.length); for (const x of v) this.i64(x); return;
      case "list": {
        const et = v.value.length ? v.type : (v.type || "end");
        this.u8(ID[et]); this.i32(v.value.length);
        for (const x of v.value) this.payload(et, x);
        return;
      }
      case "compound":
        for (const [name, tag] of Object.entries(v)) { this.u8(ID[tag.type]); this.str(name); this.payload(tag.type, tag.value); }
        this.u8(0);
        return;
      default: throw new Error(`nbt: cannot write type ${type}`);
    }
  }
  buffer() { return Buffer.concat(this.parts); }
}

/** Parse a (possibly gzipped or zlib-compressed) NBT file -> { name, tag } with tag a typed compound. */
export function parse(buf) {
  if (buf[0] === 0x1f && buf[1] === 0x8b) buf = gunzipSync(buf);
  else if (buf[0] === 0x78) buf = inflateSync(buf);
  const r = new Reader(buf);
  const type = TYPES[r.u8()];
  if (type !== "compound") throw new Error("nbt: root is not a compound");
  const name = r.str();
  return { name, tag: { type: "compound", value: r.payload("compound") } };
}

/** Serialize a typed root compound (gzipped, as structure files are). */
export function write(tag, { name = "", gzip = true } = {}) {
  const w = new Writer();
  w.u8(ID.compound); w.str(name); w.payload("compound", tag.value);
  const raw = w.buffer();
  return gzip ? gzipSync(raw) : raw;
}

/** Typed tree -> plain JS (lists -> arrays, compounds -> objects, longs stay BigInt). */
export function simplify(tag) {
  if (tag.type === "compound") return Object.fromEntries(Object.entries(tag.value).map(([k, v]) => [k, simplify(v)]));
  if (tag.type === "list") return tag.value.value.map((x) => simplify({ type: tag.value.type, value: x }));
  return tag.value;
}

/** Typed-tag builders for writing. */
export const t = {
  byte: (v) => ({ type: "byte", value: v }),
  short: (v) => ({ type: "short", value: v }),
  int: (v) => ({ type: "int", value: v }),
  long: (v) => ({ type: "long", value: BigInt(v) }),
  float: (v) => ({ type: "float", value: v }),
  double: (v) => ({ type: "double", value: v }),
  string: (v) => ({ type: "string", value: v }),
  compound: (obj) => ({ type: "compound", value: obj }),
  /** list of one element type; `values` are payloads (for compounds: plain objects of typed tags). */
  list: (type, values) => ({ type: "list", value: { type, value: values } }),
};
