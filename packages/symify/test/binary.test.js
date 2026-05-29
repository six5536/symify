"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const { selectPackage, binaryName, resolveBinary } = require("../lib/binary");

test("selectPackage maps supported platforms", () => {
  assert.strictEqual(selectPackage("linux", "x64"), "@six5536/symify-linux-x64");
  assert.strictEqual(selectPackage("darwin", "arm64"), "@six5536/symify-darwin-arm64");
});

test("selectPackage returns null for unsupported platforms", () => {
  assert.strictEqual(selectPackage("win32", "x64"), null);
  assert.strictEqual(selectPackage("linux", "riscv64"), null);
});

test("binaryName appends .exe only on Windows", () => {
  assert.strictEqual(binaryName("linux"), "symify");
  assert.strictEqual(binaryName("win32"), "symify.exe");
});

test("resolveBinary returns the resolved path for a supported platform", () => {
  const fakeResolve = (spec) => `/fake/node_modules/${spec}`;
  assert.strictEqual(
    resolveBinary("linux", "x64", fakeResolve),
    "/fake/node_modules/@six5536/symify-linux-x64/bin/symify",
  );
});

test("resolveBinary errors clearly on unsupported platform", () => {
  assert.throws(() => resolveBinary("win32", "x64", () => "unused"), /No prebuilt symify binary for win32-x64/);
});

test("resolveBinary errors clearly when the platform package is missing", () => {
  const throwing = () => {
    throw new Error("Cannot find module");
  };
  assert.throws(
    () => resolveBinary("linux", "x64", throwing),
    /platform package "@six5536\/symify-linux-x64" is not installed/,
  );
});
