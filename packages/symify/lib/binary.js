"use strict";

// Maps the host platform to the prebuilt binary package and resolves the binary
// path. Logic is dependency-injected (platform, arch, requireResolve) so it is
// unit-testable without the platform packages actually being installed.

const PACKAGES = {
  "linux x64": "@six5536/symify-linux-x64",
  "linux arm64": "@six5536/symify-linux-arm64",
  "darwin x64": "@six5536/symify-darwin-x64",
  "darwin arm64": "@six5536/symify-darwin-arm64",
};

const SUPPORTED = Object.keys(PACKAGES)
  .map((k) => k.replace(" ", "-"))
  .join(", ");

/** Return the platform package name for a platform/arch, or null if unsupported. */
function selectPackage(platform, arch) {
  return PACKAGES[`${platform} ${arch}`] || null;
}

/** The binary file name inside a platform package. */
function binaryName(platform) {
  return platform === "win32" ? "symify.exe" : "symify";
}

/**
 * Resolve the absolute path to the prebuilt binary for the given platform/arch.
 * Throws a clear, actionable error when the platform is unsupported or the
 * platform package was not installed. `requireResolve` defaults to the real
 * `require.resolve`.
 */
function resolveBinary(platform, arch, requireResolve = require.resolve) {
  const pkg = selectPackage(platform, arch);
  if (!pkg) {
    throw new Error(
      `No prebuilt symify binary for ${platform}-${arch}.\n` +
        `Supported platforms: ${SUPPORTED}.\n` +
        `Install from source instead: cargo install symify`,
    );
  }
  try {
    return requireResolve(`${pkg}/bin/${binaryName(platform)}`);
  } catch {
    throw new Error(
      `The symify platform package "${pkg}" is not installed.\n` +
        `This usually means an npm install issue (optional dependencies were skipped).\n` +
        `Try reinstalling, or install from source: cargo install symify`,
    );
  }
}

module.exports = { selectPackage, binaryName, resolveBinary, PACKAGES, SUPPORTED };
