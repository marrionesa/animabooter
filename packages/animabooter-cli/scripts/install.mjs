import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import tar from "tar";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(await readFile(join(packageDir, "package.json"), "utf8"));

if (process.env.ANIMABOOTER_SKIP_DOWNLOAD === "1") {
  process.exit(0);
}

const target = getTarget();
const extension = process.platform === "win32" ? ".exe" : "";
const asset = `animabooter-cli-${target}.tar.gz`;
const checksumAsset = "animabooter-cli-checksums.txt";
const releaseBase = `https://github.com/marrionesa/animabooter/releases/download/v${packageJson.version}`;
const workDir = join(tmpdir(), `animabooter-${process.pid}`);
const archive = join(workDir, asset);
const binaryDir = join(packageDir, ".bin");
const binary = join(binaryDir, `animabooter${extension}`);

try {
  await mkdir(workDir, { recursive: true });
  await download(`${releaseBase}/${asset}`, archive);
  const checksums = await fetchText(`${releaseBase}/${checksumAsset}`);
  verifyChecksum(checksums, asset, await readFile(archive));

  const extractedDir = join(workDir, "extracted");
  await mkdir(extractedDir, { recursive: true });
  await tar.x({ file: archive, cwd: extractedDir, strict: true });

  const extractedBinary = join(extractedDir, `animabooter${extension}`);
  await mkdir(binaryDir, { recursive: true });
  await rm(binary, { force: true });
  await copyFile(extractedBinary, binary);
  if (process.platform !== "win32") {
    await chmod(binary, 0o755);
  }
} catch (error) {
  console.error(`AnimaBooter binary installation failed: ${error.message}`);
  console.error(`Expected release asset: ${releaseBase}/${asset}`);
  process.exit(1);
} finally {
  await rm(workDir, { recursive: true, force: true });
}

function getTarget() {
  if (process.platform === "linux" && process.arch === "x64") return "linux-x64";
  if (process.platform === "darwin" && process.arch === "x64") return "darwin-x64";
  if (process.platform === "darwin" && process.arch === "arm64") return "darwin-arm64";
  if (process.platform === "win32" && process.arch === "x64") return "windows-x64";
  throw new Error(`Unsupported platform: ${process.platform}-${process.arch}`);
}

async function download(url, destination) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`download failed with HTTP ${response.status}`);
  }
  await writeFile(destination, Buffer.from(await response.arrayBuffer()));
}

async function fetchText(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`checksum download failed with HTTP ${response.status}`);
  }
  return response.text();
}

function verifyChecksum(checksums, filename, contents) {
  const line = checksums
    .split("\n")
    .map((value) => value.trim())
    .find((value) => value.endsWith(` ${filename}`) || value.endsWith(` *${filename}`));
  const expected = line?.split(/\s+/)[0];
  const actual = createHash("sha256").update(contents).digest("hex");
  if (!expected || expected !== actual) {
    throw new Error(`checksum mismatch for ${filename}`);
  }
}