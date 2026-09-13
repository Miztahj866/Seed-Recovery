// main.js — no imports, no bundler, no network calls. Uses window.__TAURI__
// (injected by Tauri because withGlobalTauri is set in tauri.conf.json) to
// call into the Rust backend commands defined in src-tauri/src/main.rs.

const { invoke } = window.__TAURI__.tauri;

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
    out.innerHTML = lines
      .map((l) =>
        l.startsWith("✓")
          ? `<span class="good">${l}</span>`
          : l.startsWith("✗")
          ? `<span class="bad">${l}</span>`
          : l
      )
      .join("\n");
  } catch (e) {
    out.innerHTML = `<span class="bad">${e}</span>`;
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

document.getElementById("missing-run").addEventListener("click", async () => {
  const out = document.getElementById("missing-output");
  const words = splitWords(missingInput.value);
  if (!words.includes("?")) {
    out.textContent = "Mark at least one unknown word with '?'.";
    return;
  }
  const licenseKey = missingLicenseKey.value.trim() || null;
  out.textContent = "Searching... this may take a moment.";
  try {
    let result;
    try {
      result = await invoke("search_missing", { words, force: false, licenseKey });
    } catch (warnMsg) {
      if (String(warnMsg).includes("requires a paid license")) {
        out.innerHTML = `<span class="bad">${warnMsg}</span>`;
        return;
      }
      if (!confirm(`${warnMsg}\n\nContinue anyway?`)) {
        out.textContent = "Cancelled.";
        return;
      }
      result = await invoke("search_missing", { words, force: true, licenseKey });
    }
    const lines = [`Checked ${result.checked.toLocaleString()} combinations.`, ""];
    if (result.matches.length > 0) {
      lines.push(`Found ${result.matches.length} candidate(s) passing the checksum:`);
      for (const m of result.matches) lines.push(`  ${renderPhrase(m)}`);
      lines.push("");
      lines.push(
        "A passing checksum means VALID BIP39, not necessarily YOUR phrase. Verify against a known address."
      );
    } else {
      lines.push("No valid combinations found. Double-check the known words for typos.");
    }
    out.textContent = lines.join("\n");
  } catch (e) {
    out.innerHTML = `<span class="bad">${e}</span>`;
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
  out.textContent = "Searching... this may take a moment.";
  try {
    let result;
    try {
      result = await invoke("search_reorder", { words, force: false });
    } catch (warnMsg) {
      if (!confirm(`${warnMsg}\n\nContinue anyway?`)) {
        out.textContent = "Cancelled.";
        return;
      }
      result = await invoke("search_reorder", { words, force: true });
    }
    const lines = [`Checked ${result.checked.toLocaleString()} orderings.`, ""];
    if (result.matches.length > 0) {
      lines.push(`Found ${result.matches.length} candidate(s) passing the checksum:`);
      for (const m of result.matches) lines.push(`  ${renderPhrase(m)}`);
    } else {
      lines.push("No valid orderings found among the words provided.");
    }
    out.textContent = lines.join("\n");
  } catch (e) {
    out.innerHTML = `<span class="bad">${e}</span>`;
  }
});
