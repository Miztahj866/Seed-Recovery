// main.js — no imports, no bundler, no network calls. Uses window.__TAURI__
// (injected by Tauri because withGlobalTauri is set in tauri.conf.json) to
// call into the Rust backend commands defined in src-tauri/src/main.rs.

const { invoke } = window.__TAURI__.tauri;
const { listen } = window.__TAURI__.event;

// --- Tab switching ---
document.querySelectorAll(".tab-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".tab-btn").forEach((b) => b.classList.remove("active"));
    document.querySelectorAll(".tab-panel").forEach((p) => p.classList.remove("active"));
    btn.classList.add("active");
    document.getElementById(`tab-${btn.dataset.tab}`).classList.add("active");
  });
});

function splitWords(text) {
  return text.trim().split(/\s+/).filter(Boolean);
}

function renderPhrase(words) {
  return words.join(" ");
}

// Show an error as plain text. Error messages can echo back words the user
// typed, so they never go through innerHTML.
function showError(out, message) {
  const span = document.createElement("span");
  span.className = "bad";
  span.textContent = String(message);
  out.replaceChildren(span);
}

// Show lines of text, coloring lines that start with ✓ (good) or ✗ (bad).
// Builds text nodes only, never HTML.
function renderLines(out, lines) {
  out.replaceChildren(
    ...lines.flatMap((l, i) => {
      const node = document.createElement("span");
      if (l.startsWith("✓")) node.className = "good";
      else if (l.startsWith("✗")) node.className = "bad";
      node.textContent = l;
      return i < lines.length - 1 ? [node, "\n"] : [node];
    })
  );
}

function formatDuration(seconds) {
  if (!isFinite(seconds)) return "unknown";
  if (seconds < 1) return "under a second";
  if (seconds < 60) return `${Math.round(seconds)} s`;
  if (seconds < 3600) {
    const m = Math.floor(seconds / 60);
    const s = Math.round(seconds % 60);
    return s ? `${m} min ${s} s` : `${m} min`;
  }
  if (seconds < 86400) {
    const h = Math.floor(seconds / 3600);
    const m = Math.round((seconds % 3600) / 60);
    return m ? `${h} h ${m} min` : `${h} h`;
  }
  const d = seconds / 86400;
  return d < 10 ? `${d.toFixed(1)} days` : `${Math.round(d).toLocaleString()} days`;
}

function formatCount(n) {
  return n < 1e15 ? Math.round(n).toLocaleString() : n.toExponential(2);
}

// --- Search progress (shared by Missing Word(s) and Word Order) ---
// Only one search runs at a time, so one listener updates whichever
// progress display is active.
let activeProgress = null;

listen("search-progress", (event) => {
  if (activeProgress) activeProgress.update(event.payload);
});

function startProgress(prefix, unit) {
  const box = document.getElementById(`${prefix}-progress`);
  const fill = box.querySelector(".progress-fill");
  const text = box.querySelector(".progress-text");
  const runBtn = document.getElementById(`${prefix}-run`);
  const cancelBtn = document.getElementById(`${prefix}-cancel`);
  const started = performance.now();

  fill.style.width = "0%";
  text.textContent = "Starting...";
  box.hidden = false;
  runBtn.disabled = true;
  cancelBtn.hidden = false;
  cancelBtn.disabled = false;
  cancelBtn.textContent = "Cancel";

  const progress = {
    update({ checked, total }) {
      const elapsed = (performance.now() - started) / 1000;
      const rate = checked / Math.max(elapsed, 0.001);
      const pct = total > 0 ? Math.min(100, (checked / total) * 100) : 0;
      const left = rate > 0 ? (total - checked) / rate : Infinity;
      fill.style.width = `${pct}%`;
      text.textContent =
        `${formatCount(checked)} of ${formatCount(total)} ${unit} checked ` +
        `(${pct < 1 ? pct.toFixed(2) : pct.toFixed(0)}%) · ` +
        `${formatCount(rate)}/s · about ${formatDuration(left)} left`;
    },
    stop() {
      box.hidden = true;
      runBtn.disabled = false;
      cancelBtn.hidden = true;
      if (activeProgress === progress) activeProgress = null;
    },
  };
  activeProgress = progress;
  return progress;
}

["missing", "reorder"].forEach((prefix) => {
  const btn = document.getElementById(`${prefix}-cancel`);
  btn.addEventListener("click", async () => {
    btn.disabled = true;
    btn.textContent = "Stopping...";
    try {
      await invoke("cancel_search");
    } catch (e) {
      // Nothing useful to show: the search result reports what happened.
    }
  });
});

// Asks a yes/no question inside the output box and resolves to true or
// false when the user clicks. Used instead of window.confirm(), which
// Tauri replaces with a version that doesn't wait for an answer (so the
// search would start without the user ever seeing the warning).
function askToContinue(out, question) {
  return new Promise((resolve) => {
    const box = document.createElement("div");
    box.className = "ask";
    const text = document.createElement("p");
    text.className = "warn";
    text.textContent = question;
    const hint = document.createElement("p");
    hint.textContent = "If you start it, you can press Cancel at any time.";
    const row = document.createElement("div");
    row.className = "actions";
    const yes = document.createElement("button");
    yes.textContent = "Continue";
    const no = document.createElement("button");
    no.className = "secondary";
    no.textContent = "Don't run";
    row.append(yes, no);
    box.append(text, hint, row);
    out.replaceChildren(box);
    no.focus();
    yes.addEventListener("click", () => resolve(true), { once: true });
    no.addEventListener("click", () => resolve(false), { once: true });
  });
}

// Runs a search command. If the backend first answers with a "this will
// take a long time" warning, asks the user before running it for real.
async function runSearch(prefix, unit, command, args, out) {
  let progress = startProgress(prefix, unit);
  try {
    try {
      return await invoke(command, { ...args, force: false });
    } catch (warnMsg) {
      const msg = String(warnMsg);
      if (!msg.includes("Pass force=true")) throw warnMsg;
      progress.stop();
      const question = msg
        .replace(" Pass force=true to proceed anyway.", "")
        .replace(/\d{5,}/g, (n) => formatCount(Number(n)));
      const runBtn = document.getElementById(`${prefix}-run`);
      runBtn.disabled = true;
      const go = await askToContinue(out, question);
      runBtn.disabled = false;
      if (!go) {
        out.textContent = "Not started.";
        return null;
      }
      out.textContent = "Searching...";
      progress = startProgress(prefix, unit);
      return await invoke(command, { ...args, force: true });
    }
  } finally {
    progress.stop();
  }
}

// Turns a search result into the text shown in the output box.
function describeResult(result, unit) {
  const lines = [];
  const time = formatDuration(result.elapsed_ms / 1000);
  if (result.cancelled) {
    const pct = result.total > 0 ? (result.checked / result.total) * 100 : 0;
    lines.push(
      `Stopped after checking ${formatCount(result.checked)} of ${formatCount(result.total)} ${unit} ` +
        `(${pct < 1 ? pct.toFixed(2) : pct.toFixed(0)}%) in ${time}.`
    );
  } else {
    lines.push(`Checked ${formatCount(result.checked)} ${unit} in ${time}.`);
  }
  lines.push("");
  if (result.match_count > 0) {
    lines.push(
      `${result.match_count.toLocaleString()} candidate(s) pass the checksum` +
        (result.cancelled ? " so far" : "") +
        (result.truncated ? `. Showing the first ${result.matches.length.toLocaleString()}:` : ":")
    );
    for (const m of result.matches) lines.push(`  ${renderPhrase(m)}`);
    lines.push("");
    lines.push(
      "A passing checksum means VALID BIP39, not necessarily YOUR phrase. Verify against a known address."
    );
  } else if (result.cancelled) {
    lines.push("No matches in the part that was searched.");
  }
  return lines;
}

// --- Fix Typos ---
document.getElementById("fix-run").addEventListener("click", async () => {
  const out = document.getElementById("fix-output");
  const words = splitWords(document.getElementById("fix-input").value);
  if (words.length === 0) {
    out.textContent = "Enter a phrase first.";
    return;
  }
  out.textContent = "Checking...";
  try {
    const result = await invoke("fix_typos", { words });
    let lines = [];
    if (result.suggestions.length > 0) {
      lines.push("Suggested corrections:");
      for (const [original, matches] of result.suggestions) {
        lines.push(`  '${original}' -> closest matches: ${matches.join(", ")}`);
      }
      lines.push("");
      lines.push(`Corrected phrase: ${renderPhrase(result.corrected_words)}`);
    } else {
      lines.push("All words are already valid wordlist entries.");
    }
    lines.push("");
    lines.push(
      result.is_valid
        ? "✓ This phrase PASSES the BIP39 checksum."
        : "✗ This phrase does NOT pass the checksum."
    );
    renderLines(out, lines);
  } catch (e) {
    showError(out, e);
  }
});

// --- Missing Word(s) ---
const missingInput = document.getElementById("missing-input");
const missingLicenseRow = document.getElementById("missing-license-row");
const missingLicenseKey = document.getElementById("missing-license-key");
const missingLicenseStatus = document.getElementById("missing-license-status");

const FREE_TIER_MAX_BLANKS = 1;

function countBlanks(words) {
  return words.filter((w) => w === "?").length;
}

// Show the license field only once it's actually needed (2+ blanks).
missingInput.addEventListener("input", () => {
  const words = splitWords(missingInput.value);
  missingLicenseRow.style.display = countBlanks(words) > FREE_TIER_MAX_BLANKS ? "flex" : "none";
});

let licenseCheckTimeout = null;
missingLicenseKey.addEventListener("input", () => {
  clearTimeout(licenseCheckTimeout);
  const key = missingLicenseKey.value.trim();
  if (!key) {
    missingLicenseStatus.textContent = "";
    missingLicenseStatus.className = "license-status";
    return;
  }
  missingLicenseStatus.textContent = "checking...";
  missingLicenseStatus.className = "license-status";
  // Debounce so we're not validating on every keystroke.
  licenseCheckTimeout = setTimeout(async () => {
    try {
      const status = await invoke("validate_license", { licenseKey: key });
      if (status.valid) {
        missingLicenseStatus.textContent = `✓ valid (${status.license_id})`;
        missingLicenseStatus.className = "license-status valid";
      } else {
        missingLicenseStatus.textContent = "✗ invalid";
        missingLicenseStatus.className = "license-status invalid";
      }
    } catch (e) {
      missingLicenseStatus.textContent = "✗ invalid";
      missingLicenseStatus.className = "license-status invalid";
    }
  }, 300);
});

// --- Known address (optional) ---
const missingAddress = document.getElementById("missing-address");
const missingAddressStatus = document.getElementById("missing-address-status");
const missingAddressExtra = document.getElementById("missing-address-extra");
const ADDRESS_HINT = missingAddressStatus.textContent;

let addressCheckTimeout = null;
missingAddress.addEventListener("input", () => {
  clearTimeout(addressCheckTimeout);
  const value = missingAddress.value.trim();
  missingAddressExtra.hidden = value === "";
  if (!value) {
    missingAddressStatus.textContent = ADDRESS_HINT;
    missingAddressStatus.className = "hint";
    return;
  }
  addressCheckTimeout = setTimeout(async () => {
    try {
      const info = await invoke("check_address", { address: value });
      missingAddressStatus.textContent = `✓ ${info.label}`;
      missingAddressStatus.className = "hint good";
    } catch (e) {
      missingAddressStatus.textContent = String(e);
      missingAddressStatus.className = "hint bad";
    }
  }, 300);
});

function addressOptions() {
  const address = missingAddress.value.trim();
  if (!address) return null;
  return {
    address,
    passphrase: document.getElementById("missing-passphrase").value,
    accounts: Number(document.getElementById("missing-accounts").value) || 1,
    addresses: Number(document.getElementById("missing-addresses").value) || 10,
    change: document.getElementById("missing-change").checked,
  };
}

// Result text when the search checked candidates against an address.
function describeAddressResult(result) {
  const time = formatDuration(result.elapsed_ms / 1000);
  const tested = `${result.match_count.toLocaleString()} possible phrase(s)`;
  if (result.found) {
    return [
      "✓ Found your phrase.",
      "",
      `  ${renderPhrase(result.found.words)}`,
      "",
      `It owns your address (wallet path ${result.found.path}). Found in ${time}.`,
      "",
      "Next: write it down carefully. Then move your funds to a new wallet with",
      "a brand-new phrase, since this one has now been typed into a computer.",
    ];
  }
  if (result.cancelled) {
    const pct = result.total > 0 ? (result.checked / result.total) * 100 : 0;
    return [
      `Stopped after ${pct < 1 ? pct.toFixed(2) : pct.toFixed(0)}% of the search, in ${time}.`,
      `Tested ${tested} against your address. None matched so far.`,
    ];
  }
  return [
    "✗ No match with these settings.",
    "",
    `Tested all ${tested} that pass the checksum against your address, in ${time}.`,
    "",
    "Things to check:",
    "  - Is the address copied exactly, and from this wallet?",
    "  - Did you set an extra password? It must be typed exactly.",
    "  - Are the '?' marks in the right places, and the other words right?",
    "  - Your address may be further in: raise the numbers under",
    "    Advanced search settings and try again.",
  ];
}

document.getElementById("missing-run").addEventListener("click", async () => {
  const out = document.getElementById("missing-output");
  const words = splitWords(missingInput.value);
  if (!words.includes("?")) {
    out.textContent = "Mark at least one unknown word with '?'.";
    return;
  }
  const licenseKey = missingLicenseKey.value.trim() || null;
  out.textContent = "Searching...";
  try {
    const result = await runSearch(
      "missing",
      "combinations",
      "search_missing",
      { words, licenseKey, addressOptions: addressOptions() },
      out
    );
    if (!result) return;
    if (result.address_checked) {
      renderLines(out, describeAddressResult(result));
      return;
    }
    const lines = describeResult(result, "combinations");
    if (result.match_count === 0 && !result.cancelled) {
      lines.push("No valid combinations found. Double-check the known words for typos.");
    }
    out.textContent = lines.join("\n");
  } catch (e) {
    showError(out, e);
  }
});

// --- Word Order ---
document.getElementById("reorder-run").addEventListener("click", async () => {
  const out = document.getElementById("reorder-output");
  const words = splitWords(document.getElementById("reorder-input").value);
  if (words.length < 2) {
    out.textContent = "Enter at least the full set of known words.";
    return;
  }
  out.textContent = "Searching...";
  try {
    const result = await runSearch("reorder", "orderings", "search_reorder", { words }, out);
    if (!result) return;
    const lines = describeResult(result, "orderings");
    if (result.match_count === 0 && !result.cancelled) {
      lines.push("No valid orderings found among the words provided.");
    }
    out.textContent = lines.join("\n");
  } catch (e) {
    showError(out, e);
  }
});
