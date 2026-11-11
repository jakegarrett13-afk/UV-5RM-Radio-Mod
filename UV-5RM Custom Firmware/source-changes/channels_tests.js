/* Simulated radio: same frame format, per-session sector erase and NOR flash behavior as the firmware.
   maxRec is the highest record the firmware lets the cable reach: 1015 with bands, 999 before them. */
class FakeRadio {
  constructor(image, maxRec = 1015) {
    this.flash = new Uint8Array(0x8000).fill(0xFF);
    this.flash.set(image);
    this.maxRec = maxRec;
    this.reset();
    this.erases = []; this.writes = 0; this.reads = 0; this.ends = 0; this.handshakes = 0;
    this.dropEvery = 0; this.corruptAddr = -1; this.dieAfterWrites = -1; this.silent = false; this.sent = 0;
    // The app data area from 0x194000: the 5120-byte satellite table the quick messages live in.
    this.data = new Uint8Array(5120).fill(0xFF); this.dataErases = 0; this.noQuick = false; this.corruptQuick = -1;
  }
  reset() { this.session = false; this.mask = 0; this.rx = []; }
  get writable() {
    const self = this;
    return this._w || (this._w = new WritableStream({ write(chunk) { self.feed(chunk); } }));
  }
  get readable() {
    const self = this;
    return this._r || (this._r = new ReadableStream({ start(c) { self.ctrl = c; } }));
  }
  async open() { this._w = null; this._r = null; void this.writable; void this.readable; this.reset(); this.isOpen = true; }
  async setSignals() {}
  async close() { this.isOpen = false; }
  reply(bytes) { if (this.isOpen) this.ctrl.enqueue(Uint8Array.from(bytes)); }
  frame(cmd, addr, data = []) {
    const body = [cmd, addr >> 8, addr & 255, data.length >> 8, data.length & 255, ...data];
    const c = crc16(body);
    return [0xA6, ...body, c >> 8, c & 255];
  }
  feed(chunk) {
    if (this.silent) return;
    this.rx.push(...chunk);
    if (!this.session) {
      const s = String.fromCharCode(...this.rx);
      if (s.includes("PROGRAMBF-5RH")) { this.rx = []; this.session = true; this.mask = 0; this.handshakes++; this.reply([0x06]); }
      else if (this.rx.length > 64) this.rx = [];
      return;
    }
    while (this.rx.length >= 8) {
      if (this.rx[0] !== 0xA6) { this.rx.shift(); continue; }
      const n = this.rx[4] << 8 | this.rx[5];
      if (this.rx.length < 8 + n) return;
      const f = this.rx.splice(0, 8 + n);
      this.handle(f[1], f[2] << 8 | f[3], f.slice(6, 6 + n), f[6 + n] << 8 | f[7 + n], f.slice(1, 6 + n));
    }
  }
  handle(cmd, addr, data, crc, body) {
    if (crc !== crc16(body)) { this.reply(this.frame(0xEE, addr, [1])); return; }
    this.sent++;
    if (this.dropEvery && this.sent % this.dropEvery === 0) return;
    if (cmd === 0x52) {
      this.reads++;
      if (addr % 32 || addr / 32 > this.maxRec) { this.reply(this.frame(0xEE, addr, [2])); return; }
      this.reply(this.frame(0x52, addr, [...this.flash.subarray(addr, addr + 32)]));
    } else if (cmd === 0x57) {
      if (data.length !== 32 || addr % 32 || addr / 32 > this.maxRec) { this.reply(this.frame(0xEE, addr, [2])); return; }
      if (this.dieAfterWrites === 0) { this.silent = true; return; }
      if (this.dieAfterWrites > 0) this.dieAfterWrites--;
      const sector = addr >> 12;
      if (!(this.mask & 1 << sector)) { this.mask |= 1 << sector; this.flash.fill(0xFF, sector << 12, (sector + 1) << 12); this.erases.push(sector); }
      this.writes++;
      for (let i = 0; i < 32; i++) {
        const v = addr + i === this.corruptAddr ? 0x00 : data[i];
        this.flash[addr + i] &= v;
      }
      this.reply(this.frame(0x57, addr));
    } else if (cmd === 0x44 && !this.noQuick) {
      // Raw flash read: the address is in 64-byte units.
      const at = addr * 64 - 0x194000, out = new Array(64).fill(0xFF);
      if (at >= 0 && at + 64 <= this.data.length) for (let i = 0; i < 64; i++) out[i] = this.data[at + i];
      this.reply(this.frame(0x44, addr, out));
    } else if (cmd === 0x53 && !this.noQuick) {
      // Satellite table: 512-byte chunks; the one at 0 erases both sectors first.
      if (data.length !== 512 || addr % 512 || addr + 512 > this.data.length) { this.reply(this.frame(0xEE, addr, [2])); return; }
      if (addr === 0) { this.data.fill(0xFF); this.dataErases++; }
      for (let i = 0; i < 512; i++) this.data[addr + i] &= addr + i === this.corruptQuick ? 0 : data[i];
      this.reply(this.frame(0x53, addr));
    } else if (cmd === 0x45) {
      this.ends++;
      this.reply(this.frame(0x45, addr));
      this.session = false;
    } else this.reply(this.frame(0xEE, addr, [3]));
  }
}

(async function () {
  const out = {};
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const dump = Uint8Array.from(atob(RADIO_DUMP_B64), c => c.charCodeAt(0));
  const downloads = [];
  const realClick = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () { if (this.download) downloads.push(this.download); else realClick.call(this); };
  window.confirm = () => true;
  const settle = async () => { await sleep(5); for (let i = 0; i < 40000 && busy; i++) await sleep(25); };
  const notice = () => $("notice").textContent;
  const useRadio = r => { port = r; };
  const closeDlgs = () => { for (const d of document.querySelectorAll("dialog")) if (d.open) d.close(); };
  const visible = id => !$(id).classList.contains("hidden");
  const builtin = () => loadCsvText(BUILTIN_CSV, "Clark County list");
  // Same bytes apart from the band bits (17-18), which the old radio dump doesn't have.
  const sameChannel = (a, b) => { for (let i = 0; i < 32; i++) if (i !== 17 && i !== 18 && a[i] !== b[i]) return false; return true; };
  const bar = name => [...document.querySelectorAll("details.sec")].find(d => d.querySelector("summary .ttl").textContent === name);
  const barButton = (name, label) => [...bar(name).querySelectorAll("summary .acts button")].find(b => b.textContent === label);
  // A radio already written from this page: the Clark County list with its bands.
  const withBands = () => { const r = channelsFromCsv(BUILTIN_CSV); return buildTable(r.chans, r.bands); };

  try {
    /* 0. Start screen. */
    out.start_visible = visible("start") && !visible("work");
    out.start_upload_disabled = $("uploadBtn").disabled && $("saveCsv").disabled;
    out.builtin_info = $("builtinInfo").textContent;

    /* 1. The encoder reproduces what CHIRP put on the radio, apart from the new band bits. */
    const fresh = channelsFromCsv(BUILTIN_CSV);
    const t0 = buildTable(fresh.chans, fresh.bands);
    let mism = [];
    for (let n = 0; n < 1000; n++) if (!sameChannel(t0.subarray(n * 32, n * 32 + 32), dump.subarray(n * 32, n * 32 + 32))) mism.push(n + 1);
    out.encoder_matches_real_radio = mism.length === 0;
    out.encoder_mismatches = mism.slice(0, 12);
    out.clark_bands = fresh.bands.map(b => b.short).join(",");
    out.ch306_bands = fresh.bands.filter((b, i) => fresh.chans.find(c => c.num === 306).bands >> i & 1).map(b => b.short).join(",");
    out.ch10_bands = fresh.chans.find(c => c.num === 10).bands;
    out.band_record = [...encodeBand({ name: "WEATHER", short: "WX" })].map(x => x.toString(16).padStart(2, "0")).join("");
    out.band_records_roundtrip = JSON.stringify(decodeBands(t0)) === JSON.stringify(fresh.bands);

    /* 2. Decode then re-encode is lossless, band bits included. */
    let rt = 0;
    for (let n = 0; n < 1000; n++) if (!isBlank(t0, n) && !sameBytes(encode(decode(t0, n)), t0.subarray(n * 32, n * 32 + 32))) rt++;
    out.decode_roundtrip_failures = rt;
    const c234 = decode(t0, 233);
    out.decode_sample = [c234.name, c234.freq, c234.duplex, c234.offset, toneLabel(c234), c234.bands].join("|");

    /* 3. Old firmware: reading stops at the band records with a clear message. */
    let r = new FakeRadio(dump, 999); useRadio(r);
    $("startRadio").click(); await settle();
    out.old_fw_notice = notice();
    out.old_fw_still_start = visible("start");

    /* 4. New firmware, no bands on the radio yet: Clark County bands by number, ready to write. */
    r = new FakeRadio(dump); useRadio(r);
    $("startRadio").click(); await settle();
    out.read_nobands_notice = notice().slice(0, 90);
    out.read_nobands_count = chans.length + "/" + bands.length + " dirty=" + dirty;
    $("uploadBtn").click(); await settle();
    out.first_write_confirm = $("confirmText").textContent;
    out.first_write_list_top = $("diffList").firstChild?.textContent.slice(0, 40);
    $("confirmYes").click(); await sleep(0); await settle();
    out.first_write_notice = notice();
    out.first_write_erased = r.erases.join(",");
    out.first_write_bands_on_radio = decodeBands(r.flash).map(b => b.short).join(",");
    out.first_write_channels_kept = (() => { for (let n = 0; n < 1000; n++) if (!sameChannel(r.flash.subarray(n * 32, n * 32 + 32), dump.subarray(n * 32, n * 32 + 32))) return n + 1; return true; })();

    /* 5. Reading a radio with bands gives them back exactly; nothing to write. */
    r = new FakeRadio(withBands()); useRadio(r);
    showStart();
    $("startRadio").click(); await settle();
    out.read_bands_notice = notice();
    out.read_identical = sameBytes(buildTable(chans, bands), withBands());
    out.read_csv_has_notes = chans.find(c => c.num === 1).note.length > 0;
    $("uploadBtn").click(); await settle();
    out.same_notice = notice(); out.same_writes = r.writes;

    /* 6. One edit: only its sector is rewritten, backup first, band records untouched. */
    r = new FakeRadio(withBands()); useRadio(r);
    setField(chans.find(c => c.num === 257), { offset: "5.000000" }); setDirty(true);
    downloads.length = 0;
    $("uploadBtn").click(); await settle();
    out.confirm_text = $("confirmText").textContent;
    $("confirmYes").click(); await sleep(0); await settle();
    out.upload_notice = notice();
    out.upload_erased = r.erases.join(",");
    out.backup_downloaded = downloads.some(d => /^radio-backup-.*\.bin$/.test(d));
    out.flash_matches_list = sameBytes(r.flash.subarray(0, TABLE_BYTES), buildTable(chans, bands));

    /* 7. Make a band, add a channel to it, rename it, write it. */
    builtin();
    $("newBand").click();
    $("bName").value = "friends"; $("bShort").value = "frnd"; $("bandOk").click();
    out.new_band = bands.slice(-1).map(b => b.name + "/" + b.short)[0] + " | " + notice().slice(0, 30);
    out.new_band_bar = !!bar("FRIENDS") && bar("FRIENDS").textContent.includes("No channels yet");
    barButton("FRIENDS", "Add channel").click();
    out.add_title = $("editTitle").textContent + " | ticked=" + [...$("fBands").querySelectorAll("input:checked")].map(i => bands[i.value].short).join(",");
    $("fName").value = "BOB"; $("fFreq").value = "462.5625"; applyEditor();
    const bob = chans.find(c => c.name === "BOB");
    out.add_bob = bob.num + " mask=" + bob.bands + " | " + notice();
    openEditor(chans.find(c => c.num === 1));
    [...$("fBands").querySelectorAll("input")].find(i => bands[i.value].name === "FRIENDS").checked = true;
    $("okBtn").click();
    out.ch1_in_two = bands.filter((b, i) => chans.find(c => c.num === 1).bands >> i & 1).map(b => b.short).join(",");
    out.ch1_shows_twice = [...document.querySelectorAll("tbody tr")].filter(tr => tr.children[1].textContent === "001").length;
    barButton("FRIENDS", "Edit").click();
    $("bName").value = "BUDDIES"; $("bShort").value = "BUD"; $("bandOk").click();
    out.renamed = bands.slice(-1)[0].name + " bar=" + !!bar("BUDDIES");
    $("newBand").click(); $("bName").value = "weather"; $("bandOk").click();
    out.dup_band_err = $("bandErr").textContent; $("bandCancel").click();
    r = new FakeRadio(withBands()); useRadio(r);
    $("uploadBtn").click(); await settle();
    out.band_write_confirm = $("confirmText").textContent + " | " + $("diffList").firstChild.textContent.slice(-30);
    $("confirmYes").click(); await sleep(0); await settle();
    out.band_write_on_radio = decodeBands(r.flash).map(b => b.short).join(",");
    out.bob_on_radio = (() => { const c = decode(r.flash, bob.num - 1); return c.name + " mask=" + c.bands; })();

    /* 8. Delete a band: keep or delete its channels; later bands' bits shift down; undo. */
    builtin();
    const n0 = chans.length, gmrsBit = bands.findIndex(b => b.short === "GMRS");
    const murs = chans.find(c => c.num === 345);
    out.murs_before = murs.bands;
    barButton("GMRS", "Delete").click();
    out.del_dialog = $("bandDelDlg").open + " | " + $("bandDelText").textContent.slice(0, 22);
    $("bandDelKeep").click();
    out.del_keep = (chans.length === n0) + " bands=" + bands.map(b => b.short).join(",") + " murs=" + murs.bands;
    out.del_keep_murs_still_murs = bands.filter((b, i) => murs.bands >> i & 1).map(b => b.short).join(",");
    $("undoBtn").click();
    out.del_undo = bands.length === 11 && murs.bands === out.murs_before && bands[gmrsBit].short === "GMRS";
    barButton("MURS", "Delete").click();
    $("bandDelAll").click();
    out.del_all = (n0 - chans.length) + " removed, bands=" + bands.length;
    $("undoBtn").click();
    out.del_all_undo = chans.length === n0 && bands.length === 11;
    const emptyBandMade = (() => { $("newBand").click(); $("bName").value = "EMPTY"; $("bandOk").click(); barButton("EMPTY", "Delete").click(); return !$("bandDelDlg").open && !bands.some(b => b.name === "EMPTY"); })();
    out.del_empty_band_no_dialog = emptyBandMade;

    /* 9. Channels: delete with undo, scan ticks, loose channels show under Not in a band. */
    const row3 = [...document.querySelectorAll("tbody tr")].find(tr => tr.children[1].textContent === "003");
    row3.querySelector("button.x").click();
    out.row_delete = !chans.some(c => c.num === 3);
    $("undoBtn").click();
    out.row_undo = chans.some(c => c.num === 3);
    out.loose_bar = !!bar("Not in a band") && bar("Not in a band").textContent.includes("Scans only under ALL");
    bar("WEATHER").querySelector("summary input").click();
    out.band_untick = chans.filter(c => c.bands & 1).every(c => c.skip === "S");

    /* 10. Saved files keep bands; files without bands get the Clark County ones. */
    builtin();
    $("newBand").click(); $("bName").value = "HIKE"; $("bShort").value = "HIKE"; $("bandOk").click();
    openEditor(chans.find(c => c.num === 2));
    [...$("fBands").querySelectorAll("input")].find(i => bands[i.value].name === "HIKE").checked = true;
    $("okBtn").click();
    const csv = exportCsv();
    out.csv_header_end = csv.split("\r\n")[0].slice(-22);
    out.csv_row1_end = csv.split("\r\n")[1].slice(-60);
    const back = channelsFromCsv(csv);
    out.csv_roundtrip = sameBytes(buildTable(back.chans, back.bands), buildTable(chans, bands));
    out.csv_bands = back.bands.map(b => b.short).join(",");
    out.plain_csv_gets_clark = channelsFromCsv(BUILTIN_CSV).bands.length === 11;

    /* 11. Backups: new .bin keeps bands; an old 32000-byte one gets the Clark County bands. */
    closeDlgs(); showStart();
    const dt = new DataTransfer();
    dt.items.add(new File([withBands()], "radio-backup-new.bin"));
    $("fileIn").files = dt.files; $("fileIn").dispatchEvent(new Event("change"));
    for (let i = 0; i < 4000 && !notice().startsWith("Opened"); i++) await sleep(25);
    out.restore_new = chans.length + "/" + bands.length + " identical=" + sameBytes(buildTable(chans, bands), withBands());
    showStart(); setNotice("");
    const dt2 = new DataTransfer();
    dt2.items.add(new File([dump.slice(0, 32000)], "radio-backup-old.bin"));
    $("fileIn").files = dt2.files; $("fileIn").dispatchEvent(new Event("change"));
    for (let i = 0; i < 4000 && !notice().startsWith("Opened"); i++) await sleep(25);
    out.restore_old = chans.length + "/" + bands.length;

    /* 12. Write safety still holds. */
    builtin();
    setField(chans.find(c => c.num === 1), { name: "WX HOME" });
    r = new FakeRadio(withBands()); r.dropEvery = 97; useRadio(r);
    $("uploadBtn").click(); await settle();
    if ($("confirmDlg").open) { $("confirmYes").click(); await sleep(0); await settle(); }
    out.retry_notice = notice(); out.retry_ch1 = decode(r.flash, 0).name;
    builtin();
    setField(chans.find(c => c.num === 1), { name: "WX HOME" });
    r = new FakeRadio(withBands()); r.corruptAddr = 21; useRadio(r);
    $("uploadBtn").click(); await settle();
    $("confirmYes").click(); await sleep(0); await settle();
    out.verify_fail_notice = notice().slice(0, 45);
    builtin();
    setField(chans.find(c => c.num === 1), { name: "WX HOME" });
    r = new FakeRadio(withBands()); r.dieAfterWrites = 20; useRadio(r);
    downloads.length = 0;
    $("uploadBtn").click(); await settle();
    $("confirmYes").click(); await sleep(0); await settle();
    out.die_backup = downloads.length > 0 && notice().includes("interrupted");
    builtin();
    setField(chans.find(c => c.num === 2), { name: "WX TWO" });
    r = new FakeRadio(withBands()); useRadio(r);
    $("uploadBtn").click(); await settle();
    r.flash[20 * 32 + 20] ^= 0x7F;
    $("confirmYes").click(); await sleep(0); await settle();
    out.stale_blocked = r.writes === 0 && notice().includes("changed since");
    builtin();
    chans = chans.filter(c => !(c.num >= 129 && c.num <= 256));
    r = new FakeRadio(withBands()); useRadio(r);
    $("uploadBtn").click(); await settle();
    $("confirmYes").click(); await sleep(0); await settle();
    out.sector_blank = (() => { for (let i = 128 * 32; i < 256 * 32; i++) if (r.flash[i] !== 0xFF) return false; return true; })();
    showStart();
    r = new FakeRadio(withBands()); r.silent = true; useRadio(r);
    $("startRadio").click(); await settle();
    out.off_notice = notice().slice(0, 30);

    /* 13. Editor basics. */
    closeDlgs(); builtin();
    openEditor(chans.find(c => c.num === 234));
    out.editor_tones = toneText(Number($("fTx").value)) + "/" + toneText(Number($("fRx").value));
    $("fFreq").value = "9999"; applyEditor();
    out.editor_err = $("formErr").textContent;
    $("fFreq").value = "147.180"; $("fNum").value = "1"; applyEditor();
    out.editor_dup_err = $("formErr").textContent;
    $("cancelBtn").click();
    out.validate_name = validate({ num: 1, freq: "146", duplex: "off", rxc: 0, txc: 0, name: "é" });
    out.tx_allowed_303 = txAllowed(chans.find(c => c.num === 303));

    /* 14. Quick messages for TEXT. */
    closeDlgs(); showStart();
    out.quick_defaults = QUICK_DEFAULTS.length + " " + QUICK_DEFAULTS[0] + " | " + QUICK_DEFAULTS[9];
    $("startQuick").click();
    const qIn = () => [...$("qList").querySelectorAll("input")];
    out.quick_open = $("quickDlg").open + " rows=" + qIn().length + " first=" + qIn()[0].value + " tenth_num=" + $("qList").children[9].value;
    r = new FakeRadio(withBands()); useRadio(r);
    $("qRead").click(); await settle();
    out.quick_read_blank = qIn().map(i => i.value).join("/") === QUICK_DEFAULTS.join("/");
    out.quick_read_blank_info = $("quickInfo").textContent.slice(0, 40);
    qIn()[0].value = "meet at camp é"; qIn()[0].dispatchEvent(new Event("input"));
    qIn()[1].value = ""; qIn()[1].dispatchEvent(new Event("input"));
    qIn()[9].value = "X".repeat(50); qIn()[9].dispatchEvent(new Event("input"));
    out.quick_clean = qIn()[0].value + "|" + $("qList").children[0].querySelector(".qcount").textContent + "|changed=" + $("qList").children[0].classList.contains("changed");
    $("qWrite").click(); await settle();
    out.quick_write = notice() + " | dlg_open=" + $("quickDlg").open + " erases=" + r.dataErases + " ends=" + r.ends;
    const rec = i => [...r.data.subarray(i * 64, i * 64 + 64)];
    out.quick_rec0 = String.fromCharCode(...rec(0).slice(0, 15)) + " len=" + rec(0)[1];
    out.quick_rec1 = rec(1).slice(0, 3).join(",");
    out.quick_rec9 = rec(9)[1] + " tail=" + rec(9).slice(52).every(b => b === 0xFF);
    out.quick_channels_untouched = r.writes === 0 && sameBytes(r.flash.subarray(0, TABLE_BYTES), withBands());
    // Read it back on a fresh radio holding those bytes, with one record damaged.
    const r2 = new FakeRadio(withBands()); r2.data.set(r.data); r2.data[3 * 64 + 1] = 200; r2.data[4 * 64 + 3] = 7; useRadio(r2);
    quickOnRadio = null; $("startQuick").click();
    $("qRead").click(); await settle();
    out.quick_read_back = qIn().map(i => i.value).slice(0, 5).join("/") + " | " + $("quickInfo").textContent;
    // A bad read-back is reported, and the dialog stays open.
    const r3 = new FakeRadio(withBands()); r3.corruptQuick = 64 + 2; useRadio(r3);
    qIn()[1].value = "HELLO"; qIn()[1].dispatchEvent(new Event("input"));
    $("qWrite").click(); await settle();
    out.quick_verify_fail = $("quickErr").textContent.slice(0, 40) + " | open=" + $("quickDlg").open;
    // Older firmware without these commands.
    const r4 = new FakeRadio(withBands()); r4.noQuick = true; useRadio(r4);
    $("qRead").click(); await settle();
    out.quick_old_fw = $("quickErr").textContent.slice(0, 50);
    $("qDefaults").click();
    out.quick_defaults_button = qIn().map(i => i.value).join("/") === QUICK_DEFAULTS.join("/");
    $("qClose").click();
    out.quick_header_button = (() => { builtin(); $("quickBtn").click(); const o = $("quickDlg").open; $("qClose").click(); return o && visible("work"); })();

    const bgRule = getComputedStyle(document.body, "::before").backgroundImage;
    out.skull_background = bgRule.startsWith('url("data:image/png');
    out.done = true;
  } catch (e) { out.ERROR = e.stack || String(e); }
  $("testout").textContent = "RESULT" + JSON.stringify(out) + "ENDRESULT";
})();
