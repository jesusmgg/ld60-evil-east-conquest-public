"use strict";

(function () {
    var worker = null;
    var ready_state = 0;
    var persistent_state = -1;
    var is_resetting = false;

    var boot_blob = null;
    var load_slot = null;
    var closed_queue = [];
    var closed_record_size = 0;

    var name_decoder = new TextDecoder();

    var storage_free = -1;
    function refresh_storage_estimate() {
        if (typeof navigator.storage === "undefined" || typeof navigator.storage.estimate !== "function") {
            return;
        }
        navigator.storage.estimate().then(function (estimate) {
            storage_free = estimate.quota - estimate.usage;
        }, function () {});
    }

    var worker_code = `
"use strict";
var HISTORY_TIER_COUNT = 0;
var HISTORY_FILE_BYTES = 0;
var HISTORY_FILE_MAGIC = [];
var HISTORY_TIER_RATIO = [];
var HISTORY_STATE_MAGIC = [];
var HISTORY_MANIFEST_NAME = "";
var HISTORY_SECTORS_NAME = "";
var HISTORY_STATE_NAME = "";
var HISTORY_STATE_PREV_SUFFIX = "";
var HISTORY_LEVELS_DIR_NAME = "";
var HISTORY_FILE_EXT = "";
var HISTORY_FILE_HEADER_BYTES = 0;
var HISTORY_FILE_PATCH_OFFSET = 0;
var HISTORY_MANIFEST_SAVE_ID_OFFSET = 0;
var HISTORY_FILE_TIER_OFFSET = 0;
var HISTORY_FILE_TURN_FIRST_OFFSET = 0;
var HISTORY_CHUNK_TURN_FIRST_OFFSET = 0;
var HISTORY_CHUNK_TURN_LAST_OFFSET = 0;
var HISTORY_MANIFEST_BYTES = 0;
var HISTORY_SNAPSHOTS_NAME = "";
var text_encoder = new TextEncoder();
var text_decoder = new TextDecoder();

var root_handle = null;
var dir_handle = null;
var failed_dir_name = null;
var format_version = 0;
var open_files = [];
var next_sequence = [];
var is_head_written_this_session = false;
var is_inherited_head_good = false;

var tier_queue = [];
var head_state_queue = Promise.resolve();
var is_reset = false;

self.onmessage = function (event) {
    var message = event.data;
    if (is_reset) return;
    switch (message.type) {
        case "reset_game":
            is_reset = true;
            Promise.allSettled(tier_queue.concat([head_state_queue])).then(reset_game);
            break;
        case "init":
            HISTORY_TIER_COUNT = message.tier_count;
            HISTORY_FILE_BYTES = message.file_bytes;
            HISTORY_FILE_MAGIC = message.file_magic;
            HISTORY_TIER_RATIO = message.tier_ratio;
            HISTORY_STATE_MAGIC = message.state_magic;
            HISTORY_MANIFEST_NAME = message.manifest_name;
            HISTORY_SECTORS_NAME = message.sectors_name;
            HISTORY_STATE_NAME = message.state_name;
            HISTORY_STATE_PREV_SUFFIX = message.state_prev_suffix;
            HISTORY_LEVELS_DIR_NAME = message.levels_dir_name;
            HISTORY_FILE_EXT = message.file_ext;
            HISTORY_FILE_HEADER_BYTES = message.file_header_bytes;
            HISTORY_FILE_PATCH_OFFSET = message.file_patch_offset;
            HISTORY_MANIFEST_SAVE_ID_OFFSET = message.manifest_save_id_offset;
            HISTORY_FILE_TIER_OFFSET = message.file_tier_offset;
            HISTORY_FILE_TURN_FIRST_OFFSET = message.file_turn_first_offset;
            HISTORY_CHUNK_TURN_FIRST_OFFSET = message.chunk_turn_first_offset;
            HISTORY_CHUNK_TURN_LAST_OFFSET = message.chunk_turn_last_offset;
            HISTORY_MANIFEST_BYTES = message.manifest_size;
            HISTORY_SNAPSHOTS_NAME = message.snapshots_name;
            open_files = new Array(HISTORY_TIER_COUNT).fill(null);
            next_sequence = new Array(HISTORY_TIER_COUNT).fill(0);
            var probe_done = probe_opfs(message);
            tier_queue = new Array(HISTORY_TIER_COUNT).fill(probe_done);
            head_state_queue = probe_done;
            break;
        case "clear_failed_save":
            var clear_done = clear_failed_save();
            tier_queue = new Array(HISTORY_TIER_COUNT).fill(clear_done);
            head_state_queue = clear_done;
            break;
        case "seed_sequences":
            for (var s = 0; s < message.seeds.length; s++) {
                next_sequence[s] = message.seeds[s];
            }
            break;
        case "mark_head_good":
            is_inherited_head_good = true;
            break;
        case "write_chunk":
            tier_queue[message.tier] = tier_queue[message.tier].then(function () {
                return write_chunk(message.tier, message.bytes);
            });
            break;
        case "write_head_state":
            head_state_queue = head_state_queue.then(function () {
                return write_head_state(message.bytes);
            });
            break;
        case "write_snapshots":
            head_state_queue = head_state_queue.then(function () {
                return write_snapshots(message.bytes);
            });
            break;
        case "write_bundle":
            head_state_queue = head_state_queue.then(function () {
                return write_bundle(message.bytes);
            });
            break;
        case "commit_fresh":
            var commit_promise = tier_queue[0].then(function () {
                return commit_fresh(message.manifest_bytes, message.sectors_bytes, message.bundle_bytes, message.head_bytes);
            });
            for (var t = 0; t < HISTORY_TIER_COUNT; t++) {
                tier_queue[t] = tier_queue[t].then(function () {
                    return commit_promise;
                });
            }
            head_state_queue = head_state_queue.then(function () {
                return commit_promise;
            });
            break;
        case "request_load":
            tier_queue[message.tier] = tier_queue[message.tier].then(function () {
                return request_load(message.tier, message.turn_first, message.sequence);
            });
            break;
        case "delete_file":
            tier_queue[message.tier] = tier_queue[message.tier].then(function () {
                return delete_file(message.tier, message.turn_first, message.sequence);
            });
            break;
        case "republish":
            tier_queue[message.tier] = tier_queue[message.tier].then(function () {
                return republish(message.tier, message.bytes);
            });
            break;
    }
};

function put_u32(view, offset, value) {
    view.setUint32(offset, value, true);
}

function magic_matches(bytes, magic) {
    if (bytes.length < 8) return false;
    for (var i = 0; i < 8; i++) {
        if (bytes[i] !== magic[i]) return false;
    }
    return true;
}

function manifest_save_id(bytes) {
    if (bytes.length < HISTORY_MANIFEST_BYTES) {
        return null;
    }
    var view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    var low = view.getBigUint64(HISTORY_MANIFEST_SAVE_ID_OFFSET, true);
    var high = view.getBigUint64(HISTORY_MANIFEST_SAVE_ID_OFFSET + 8, true);
    return (high << 64n) | low;
}

async function read_whole_entry(fh) {
    var handle = null;
    try {
        handle = await fh.createSyncAccessHandle();
        var size = handle.getSize();
        var bytes = new Uint8Array(size);
        if (size > 0) {
            handle.read(bytes, { at: 0 });
        }
        return bytes;
    } catch (e) {
        return new Uint8Array(0);
    } finally {
        if (handle !== null) handle.close();
    }
}

async function read_whole_file(dir, name) {
    try {
        return await read_whole_entry(await dir.getFileHandle(name));
    } catch (e) {
        return new Uint8Array(0);
    }
}

async function read_head_file(name) {
    var handle = null;
    try {
        var fh = await dir_handle.getFileHandle(name);
        handle = await fh.createSyncAccessHandle();
        var size = handle.getSize();
        if (size >= 8) {
            var bytes = new Uint8Array(size);
            handle.read(bytes, { at: 0 });
            if (magic_matches(bytes, HISTORY_STATE_MAGIC)) {
                return bytes;
            }
        }
    } catch (e) {
    } finally {
        if (handle !== null) handle.close();
    }
    return new Uint8Array(0);
}

async function read_bundle_container() {
    var levels_dir;
    try {
        levels_dir = await dir_handle.getDirectoryHandle(HISTORY_LEVELS_DIR_NAME);
    } catch (e) {
        return new Uint8Array(0);
    }
    var names = [];
    var datas = [];
    for await (var entry of levels_dir.values()) {
        if (entry.kind !== "file") continue;
        names.push(text_encoder.encode(entry.name));
        datas.push(await read_whole_entry(entry));
    }
    if (names.length === 0) {
        return new Uint8Array(0);
    }
    var total = 4;
    for (var i = 0; i < names.length; i++) {
        total += 2 + names[i].length + 4 + datas[i].length;
    }
    var buffer = new ArrayBuffer(total);
    var out = new Uint8Array(buffer);
    var view = new DataView(buffer);
    view.setUint32(0, names.length, true);
    var offset = 4;
    for (var j = 0; j < names.length; j++) {
        view.setUint16(offset, names[j].length, true); offset += 2;
        out.set(names[j], offset); offset += names[j].length;
        view.setUint32(offset, datas[j].length, true); offset += 4;
        out.set(datas[j], offset); offset += datas[j].length;
    }
    return out;
}

async function read_files_container() {
    var names = [];
    var byte_lengths = [];
    var headers = [];
    for await (var entry of dir_handle.values()) {
        if (entry.kind !== "file" || !entry.name.endsWith("." + HISTORY_FILE_EXT)) continue;
        var handle = null;
        try {
            handle = await entry.createSyncAccessHandle();
            var byte_length = handle.getSize();
            if (byte_length < HISTORY_FILE_HEADER_BYTES) continue;
            var header = new Uint8Array(HISTORY_FILE_HEADER_BYTES);
            handle.read(header, { at: 0 });
            names.push(text_encoder.encode(entry.name));
            byte_lengths.push(byte_length);
            headers.push(header);
        } catch (e) {
        } finally {
            if (handle !== null) handle.close();
        }
    }
    var total = 4;
    for (var i = 0; i < names.length; i++) {
        total += 2 + names[i].length + 4 + HISTORY_FILE_HEADER_BYTES;
    }
    var buffer = new ArrayBuffer(total);
    var out = new Uint8Array(buffer);
    var view = new DataView(buffer);
    view.setUint32(0, names.length, true);
    var offset = 4;
    for (var j = 0; j < names.length; j++) {
        view.setUint16(offset, names[j].length, true); offset += 2;
        out.set(names[j], offset); offset += names[j].length;
        view.setUint32(offset, byte_lengths[j], true); offset += 4;
        out.set(headers[j], offset); offset += HISTORY_FILE_HEADER_BYTES;
    }
    return out;
}

function build_boot_blob(manifest, sectors, head, head_prev, bundle, files, snapshots, snapshots_prev) {
    var total = 1 + (4 + manifest.length) + (4 + sectors.length) + (4 + head.length) +
        (4 + head_prev.length) + (4 + bundle.length) + (4 + files.length) +
        (4 + snapshots.length) + (4 + snapshots_prev.length);
    var buffer = new ArrayBuffer(total);
    var out = new Uint8Array(buffer);
    var view = new DataView(buffer);
    out[0] = 1;
    var offset = 1;
    view.setUint32(offset, manifest.length, true); offset += 4;
    out.set(manifest, offset); offset += manifest.length;
    view.setUint32(offset, sectors.length, true); offset += 4;
    out.set(sectors, offset); offset += sectors.length;
    view.setUint32(offset, head.length, true); offset += 4;
    out.set(head, offset); offset += head.length;
    view.setUint32(offset, head_prev.length, true); offset += 4;
    out.set(head_prev, offset); offset += head_prev.length;
    view.setUint32(offset, bundle.length, true); offset += 4;
    out.set(bundle, offset); offset += bundle.length;
    view.setUint32(offset, files.length, true); offset += 4;
    out.set(files, offset); offset += files.length;
    view.setUint32(offset, snapshots.length, true); offset += 4;
    out.set(snapshots, offset); offset += snapshots.length;
    view.setUint32(offset, snapshots_prev.length, true); offset += 4;
    out.set(snapshots_prev, offset);
    return out;
}

async function is_aborted_creation(dir_entry) {
    try {
        for await (var entry of dir_entry.values()) {
            if (entry.kind !== "file") continue;
            if (entry.name === HISTORY_STATE_NAME
                || entry.name === HISTORY_STATE_NAME + HISTORY_STATE_PREV_SUFFIX
                || entry.name.endsWith("." + HISTORY_FILE_EXT)) {
                return false;
            }
        }
        return true;
    } catch (e) {
        return false;
    }
}

async function open_storage_root() {
    try {
        var opfs_root = await navigator.storage.getDirectory();
        var probe_file = await opfs_root.getFileHandle(STORAGE_PROBE_NAME, { create: true });
        var probe_handle = await probe_file.createSyncAccessHandle();
        probe_handle.close();
        await opfs_root.removeEntry(STORAGE_PROBE_NAME);
        return opfs_root;
    } catch (e) {
        return await open_idb_root();
    }
}

var STORAGE_PROBE_NAME = "storage_probe";
var IDB_NAME = "evil_east_conquest_files";
var IDB_PAGE_BYTES = 64 * 1024;
var idb = null;

function idb_request(request) {
    return new Promise(function (resolve, reject) {
        request.onsuccess = function () { resolve(request.result); };
        request.onerror = function () { reject(request.error); };
    });
}

function idb_transaction_done(transaction) {
    return new Promise(function (resolve, reject) {
        transaction.oncomplete = function () { resolve(); };
        transaction.onerror = function () { reject(transaction.error); };
        transaction.onabort = function () { reject(transaction.error); };
    });
}

function idb_report_error() {
    self.postMessage({ type: "error" });
}

function idb_child_path(parent, name) {
    return parent === "" ? name : parent + "/" + name;
}

function idb_page_count(size) {
    return Math.ceil(size / IDB_PAGE_BYTES);
}

async function open_idb_root() {
    var request = indexedDB.open(IDB_NAME, 1);
    request.onupgradeneeded = function () {
        request.result.createObjectStore("entries");
        request.result.createObjectStore("pages");
    };
    idb = await idb_request(request);
    return new IdbDirectoryHandle("", "");
}

async function idb_get_entry(path) {
    return await idb_request(idb.transaction("entries").objectStore("entries").get(path));
}

class IdbDirectoryHandle {
    constructor(path, name) {
        this.kind = "directory";
        this.path = path;
        this.name = name;
    }

    async getDirectoryHandle(name, options) {
        var path = idb_child_path(this.path, name);
        await idb_open_entry(path, "directory", options);
        return new IdbDirectoryHandle(path, name);
    }

    async getFileHandle(name, options) {
        var path = idb_child_path(this.path, name);
        await idb_open_entry(path, "file", options);
        return new IdbFileHandle(path, name);
    }

    async *values() {
        var prefix = this.path === "" ? "" : this.path + "/";
        var store = idb.transaction("entries").objectStore("entries");
        var range = IDBKeyRange.bound(prefix, prefix + "\uffff");
        var keys_request = store.getAllKeys(range);
        var values_request = store.getAll(range);
        var keys = await idb_request(keys_request);
        var values = await idb_request(values_request);
        for (var i = 0; i < keys.length; i++) {
            var key = keys[i];
            if (key.length <= prefix.length || key.indexOf("/", prefix.length) !== -1) continue;
            var name = key.substring(prefix.length);
            if (values[i].kind === "directory") {
                yield new IdbDirectoryHandle(key, name);
            } else {
                yield new IdbFileHandle(key, name);
            }
        }
    }

    async removeEntry(name, options) {
        var path = idb_child_path(this.path, name);
        var entry = await idb_get_entry(path);
        if (entry === undefined) {
            throw new DOMException(name, "NotFoundError");
        }
        var transaction = idb.transaction(["entries", "pages"], "readwrite");
        var entries = transaction.objectStore("entries");
        var pages = transaction.objectStore("pages");
        entries.delete(path);
        pages.delete(IDBKeyRange.bound([path, 0], [path, Infinity]));
        if (entry.kind === "directory") {
            entries.delete(IDBKeyRange.bound(path + "/", path + "/\uffff"));
            pages.delete(IDBKeyRange.bound([path + "/", 0], [path + "/\uffff", Infinity]));
        }
        await idb_transaction_done(transaction);
    }
}

async function idb_open_entry(path, kind, options) {
    var entry = await idb_get_entry(path);
    if (entry === undefined) {
        if (!options || !options.create) {
            throw new DOMException(path, "NotFoundError");
        }
        var value = kind === "file" ? { kind: "file", size: 0 } : { kind: "directory" };
        var transaction = idb.transaction("entries", "readwrite");
        transaction.objectStore("entries").put(value, path);
        await idb_transaction_done(transaction);
    } else if (entry.kind !== kind) {
        throw new DOMException(path, "TypeMismatchError");
    }
}

class IdbFileHandle {
    constructor(path, name) {
        this.kind = "file";
        this.path = path;
        this.name = name;
    }

    async createSyncAccessHandle() {
        var transaction = idb.transaction(["entries", "pages"]);
        var entry_request = transaction.objectStore("entries").get(this.path);
        var pages_request = transaction.objectStore("pages").getAll(IDBKeyRange.bound([this.path, 0], [this.path, Infinity]));
        var entry = await idb_request(entry_request);
        var pages = await idb_request(pages_request);
        if (entry === undefined || entry.kind !== "file") {
            throw new DOMException(this.path, "NotFoundError");
        }
        return new IdbSyncAccessHandle(this.path, entry.size, pages);
    }
}

class IdbSyncAccessHandle {
    constructor(path, size, pages) {
        this.path = path;
        this.size = size;
        this.data = new Uint8Array(Math.max(size, IDB_PAGE_BYTES));
        for (var i = 0; i < pages.length; i++) {
            var start = pages[i].index * IDB_PAGE_BYTES;
            var length = Math.min(pages[i].bytes.length, size - start);
            if (length > 0) {
                this.data.set(pages[i].bytes.subarray(0, length), start);
            }
        }
        this.stored_page_count = idb_page_count(size);
        this.dirty_pages = new Set();
    }

    getSize() {
        return this.size;
    }

    read(buffer, options) {
        var at = options.at;
        var length = Math.max(0, Math.min(buffer.length, this.size - at));
        buffer.set(this.data.subarray(at, at + length));
        return length;
    }

    write(buffer, options) {
        var at = options.at;
        var end = at + buffer.length;
        this.reserve(end);
        this.data.set(buffer, at);
        this.mark_dirty(at, end);
        if (end > this.size) {
            this.size = end;
        }
        return buffer.length;
    }

    truncate(size) {
        if (size < this.size) {
            this.data.fill(0, size, this.size);
        } else {
            this.reserve(size);
        }
        this.mark_dirty(Math.min(size, this.size), Math.max(size, this.size));
        this.size = size;
    }

    flush() {
        var transaction = idb.transaction(["entries", "pages"], "readwrite");
        transaction.onerror = idb_report_error;
        transaction.onabort = idb_report_error;
        transaction.objectStore("entries").put({ kind: "file", size: this.size }, this.path);
        var pages = transaction.objectStore("pages");
        var page_count = idb_page_count(this.size);
        for (var index of this.dirty_pages) {
            if (index >= page_count) continue;
            var start = index * IDB_PAGE_BYTES;
            var bytes = this.data.slice(start, Math.min(start + IDB_PAGE_BYTES, this.size));
            pages.put({ index: index, bytes: bytes }, [this.path, index]);
        }
        if (this.stored_page_count > page_count) {
            pages.delete(IDBKeyRange.bound([this.path, page_count], [this.path, Infinity]));
        }
        this.stored_page_count = page_count;
        this.dirty_pages.clear();
    }

    close() {
        this.flush();
    }

    reserve(bytes) {
        if (bytes <= this.data.length) return;
        var capacity = this.data.length;
        while (capacity < bytes) {
            capacity *= 2;
        }
        var grown = new Uint8Array(capacity);
        grown.set(this.data.subarray(0, this.size));
        this.data = grown;
    }

    mark_dirty(start, end) {
        if (end <= start) return;
        var last = Math.floor((end - 1) / IDB_PAGE_BYTES);
        for (var index = Math.floor(start / IDB_PAGE_BYTES); index <= last; index++) {
            this.dirty_pages.add(index);
        }
    }
}

async function probe_opfs(message) {
    failed_dir_name = null;
    try {
        format_version = message.format_version;
        if (root_handle === null) {
            root_handle = await open_storage_root();
        }

        var candidates = [];
        for await (var entry of root_handle.values()) {
            if (entry.kind === "directory" && entry.name.startsWith("save_")) {
                candidates.push(entry);
            }
        }

        var newest = null;
        var newest_id = -1n;
        var newest_name = "";
        var newest_manifest = new Uint8Array(0);
        var orphans = [];
        for (var i = 0; i < candidates.length; i++) {
            failed_dir_name = candidates[i].name;
            var manifest_bytes = await read_whole_file(candidates[i], HISTORY_MANIFEST_NAME);
            var id = manifest_save_id(manifest_bytes);
            if (id === null) {
                if (await is_aborted_creation(candidates[i])) {
                    orphans.push(candidates[i]);
                    continue;
                }
                self.postMessage({ type: "ready", value: -1 });
                return;
            }
            if (id > newest_id || (id === newest_id && candidates[i].name > newest_name)) {
                newest = candidates[i];
                newest_id = id;
                newest_name = candidates[i].name;
                newest_manifest = manifest_bytes;
            }
        }
        failed_dir_name = newest === null ? null : newest.name;

        for (var j = 0; j < orphans.length; j++) {
            try {
                await root_handle.removeEntry(orphans[j].name, { recursive: true });
            } catch (e) {
            }
        }

        if (newest === null) {
            var fresh = new Uint8Array(1);
            fresh[0] = 0;
            self.postMessage({ type: "boot_probe", bytes: fresh.buffer }, [fresh.buffer]);
            self.postMessage({ type: "ready", value: 1 });
            return;
        }

        dir_handle = newest;
        var manifest = newest_manifest;
        var sectors = await read_whole_file(dir_handle, HISTORY_SECTORS_NAME);
        var head = await read_head_file(HISTORY_STATE_NAME);
        var head_prev = await read_head_file(HISTORY_STATE_NAME + HISTORY_STATE_PREV_SUFFIX);
        var bundle = await read_bundle_container();
        var files = await read_files_container();
        var snapshots = await read_whole_file(dir_handle, HISTORY_SNAPSHOTS_NAME);
        var snapshots_prev = await read_whole_file(dir_handle, HISTORY_SNAPSHOTS_NAME + HISTORY_STATE_PREV_SUFFIX);
        var boot = build_boot_blob(manifest, sectors, head, head_prev, bundle, files, snapshots, snapshots_prev);
        self.postMessage({ type: "boot_probe", bytes: boot.buffer }, [boot.buffer]);

        self.postMessage({ type: "ready", value: 1 });
        request_persistence();
    } catch (e) {
        self.postMessage({ type: "ready", value: -1 });
    }
}

async function clear_failed_save() {
    if (failed_dir_name === null) {
        self.postMessage({ type: "ready", value: -2 });
        return;
    }
    try {
        await root_handle.removeEntry(failed_dir_name, { recursive: true });
    } catch (e) {
        self.postMessage({ type: "ready", value: -2 });
        return;
    }
    dir_handle = null;
    await probe_opfs({ format_version: format_version });
}

async function reset_game() {
    try {
        for (var tier = 0; tier < open_files.length; tier++) {
            if (open_files[tier] !== null) {
                open_files[tier].handle.close();
                open_files[tier] = null;
            }
        }
        if (root_handle === null) return;
        var names = [];
        for await (var entry of root_handle.values()) {
            if (entry.kind === "directory" && entry.name.startsWith("save_")) {
                names.push(entry.name);
            }
        }
        for (var i = 0; i < names.length; i++) {
            try {
                await root_handle.removeEntry(names[i], { recursive: true });
            } catch (e) {
            }
        }
    } finally {
        self.postMessage({ type: "reset_done" });
    }
}

function request_persistence() {
    if (typeof navigator.storage === "undefined" || typeof navigator.storage.persist !== "function") {
        return;
    }
    navigator.storage.persist().then(function (persistent) {
        self.postMessage({ type: "persistent", value: persistent ? 1 : 0 });
    }, function () {
        self.postMessage({ type: "persistent", value: -2 });
    });
}

async function write_whole_file(dir, name, bytes) {
    var handle = null;
    try {
        var fh = await dir.getFileHandle(name, { create: true });
        handle = await fh.createSyncAccessHandle();
        handle.truncate(0);
        handle.write(bytes, { at: 0 });
        handle.flush();
    } finally {
        if (handle !== null) handle.close();
    }
}

async function commit_fresh(manifest_bytes, sectors_bytes, bundle_bytes, head_bytes) {
    try {
        var name = "save_" + Date.now().toString(36);
        dir_handle = await root_handle.getDirectoryHandle(name, { create: true });
        await write_whole_file(dir_handle, HISTORY_MANIFEST_NAME, new Uint8Array(manifest_bytes));
        await write_whole_file(dir_handle, HISTORY_SECTORS_NAME, new Uint8Array(sectors_bytes));
        await write_whole_file(dir_handle, HISTORY_STATE_NAME, new Uint8Array(head_bytes));
        is_head_written_this_session = true;
        var levels_dir = await dir_handle.getDirectoryHandle(HISTORY_LEVELS_DIR_NAME, { create: true });
        await unpack_bundle(levels_dir, new Uint8Array(bundle_bytes));
        request_persistence();
    } catch (e) {
        self.postMessage({ type: "error" });
    }
}

async function unpack_bundle(levels_dir, bundle) {
    var bview = new DataView(bundle.buffer, bundle.byteOffset, bundle.byteLength);
    var count = bundle.length >= 4 ? bview.getUint32(0, true) : 0;
    var offset = 4;
    for (var i = 0; i < count; i++) {
        var name_len = bview.getUint16(offset, true); offset += 2;
        var name_bytes = bundle.subarray(offset, offset + name_len); offset += name_len;
        var data_len = bview.getUint32(offset, true); offset += 4;
        var data = bundle.subarray(offset, offset + data_len); offset += data_len;
        await write_whole_file(levels_dir, text_decoder.decode(name_bytes), data);
    }
}

async function write_bundle(bundle_bytes) {
    try {
        var levels_dir = await dir_handle.getDirectoryHandle(HISTORY_LEVELS_DIR_NAME, { create: true });
        await unpack_bundle(levels_dir, new Uint8Array(bundle_bytes));
    } catch (e) {
        self.postMessage({ type: "error" });
    }
}

function post_file_closed(tier, sequence, turn_first, turn_last, byte_length) {
    self.postMessage({
        type: "file_closed",
        tier: tier,
        sequence: sequence,
        turn_first: turn_first,
        turn_last: turn_last,
        byte_length: byte_length
    });
}

async function request_load(tier, turn_first, sequence) {
    var handle = null;
    try {
        var fh = await dir_handle.getFileHandle(file_name(tier, turn_first, sequence));
        handle = await fh.createSyncAccessHandle();
        var size = handle.getSize();
        var bytes = new Uint8Array(size);
        if (size > 0) {
            handle.read(bytes, { at: 0 });
        }
        self.postMessage({ type: "loaded", tier: tier, sequence: sequence, bytes: bytes.buffer }, [bytes.buffer]);
    } catch (e) {
        self.postMessage({ type: "error" });
    } finally {
        if (handle !== null) handle.close();
    }
}

async function delete_file(tier, turn_first, sequence) {
    try {
        await dir_handle.removeEntry(file_name(tier, turn_first, sequence));
    } catch (e) {
        self.postMessage({ type: "error" });
    }
}

async function republish(tier, container_bytes) {
    var handle = null;
    try {
        var container = new Uint8Array(container_bytes);
        var cview = new DataView(container.buffer, container.byteOffset, container.byteLength);
        var chunk_count = cview.getUint32(0, true);
        if (chunk_count === 0) return;
        var first_len = cview.getUint32(4, true);
        var first_chunk = container.subarray(8, 8 + first_len);
        var first_view = new DataView(first_chunk.buffer, first_chunk.byteOffset, first_chunk.byteLength);
        var turn_first = first_view.getUint32(HISTORY_CHUNK_TURN_FIRST_OFFSET, true);

        var sequence = next_sequence[tier]++;
        var fh = await dir_handle.getFileHandle(file_name(tier, turn_first, sequence), { create: true });
        handle = await fh.createSyncAccessHandle();
        handle.truncate(0);
        var header = build_file_header(tier, turn_first);
        handle.write(header, { at: 0 });
        var bytes_written = header.byteLength;
        var turn_last = turn_first;
        var offset = 4;
        for (var i = 0; i < chunk_count; i++) {
            var chunk_len = cview.getUint32(offset, true); offset += 4;
            var chunk = container.subarray(offset, offset + chunk_len); offset += chunk_len;
            var chunk_view = new DataView(chunk.buffer, chunk.byteOffset, chunk.byteLength);
            turn_last = chunk_view.getUint32(HISTORY_CHUNK_TURN_LAST_OFFSET, true);
            var length_prefix = new ArrayBuffer(4);
            new DataView(length_prefix).setUint32(0, chunk_len, true);
            handle.write(new Uint8Array(length_prefix), { at: bytes_written });
            handle.write(chunk, { at: bytes_written + 4 });
            bytes_written += 4 + chunk_len;
        }
        var patch = new ArrayBuffer(8);
        var patch_view = new DataView(patch);
        patch_view.setUint32(0, turn_last, true);
        patch_view.setUint32(4, chunk_count, true);
        handle.write(new Uint8Array(patch), { at: HISTORY_FILE_PATCH_OFFSET });
        handle.flush();
        handle.close();
        handle = null;
        post_file_closed(tier, sequence, turn_first, turn_last, bytes_written);
    } catch (e) {
        self.postMessage({ type: "error" });
    } finally {
        if (handle !== null) handle.close();
    }
}

function file_name(tier, turn_first, sequence) {
    var tf = turn_first.toString().padStart(10, "0");
    var seq = sequence.toString().padStart(6, "0");
    return "t" + tier + "-" + tf + "-" + seq + "." + HISTORY_FILE_EXT;
}

function build_file_header(tier, turn_first) {
    var buffer = new ArrayBuffer(HISTORY_FILE_HEADER_BYTES);
    var bytes = new Uint8Array(buffer);
    bytes.set(HISTORY_FILE_MAGIC, 0);
    var view = new DataView(buffer);
    put_u32(view, 8, format_version);
    put_u32(view, HISTORY_FILE_TIER_OFFSET, tier);
    put_u32(view, 16, HISTORY_TIER_RATIO[tier]);
    put_u32(view, HISTORY_FILE_TURN_FIRST_OFFSET, turn_first);
    put_u32(view, HISTORY_FILE_PATCH_OFFSET, 0);
    put_u32(view, HISTORY_FILE_PATCH_OFFSET + 4, 0);
    put_u32(view, HISTORY_FILE_PATCH_OFFSET + 8, 0);
    return bytes;
}

async function write_chunk(tier, chunk_bytes) {
    try {
        var chunk = new Uint8Array(chunk_bytes);
        var chunk_view = new DataView(chunk.buffer, chunk.byteOffset, chunk.byteLength);
        var chunk_turn_first = chunk_view.getUint32(HISTORY_CHUNK_TURN_FIRST_OFFSET, true);
        var chunk_turn_last = chunk_view.getUint32(HISTORY_CHUNK_TURN_LAST_OFFSET, true);

        var state = open_files[tier];
        if (state === null) {
            var sequence = next_sequence[tier]++;
            var name = file_name(tier, chunk_turn_first, sequence);
            var fh = await dir_handle.getFileHandle(name, { create: true });
            var handle = await fh.createSyncAccessHandle();
            try {
                handle.truncate(0);
                var header = build_file_header(tier, chunk_turn_first);
                handle.write(header, { at: 0 });
            } catch (create_error) {
                handle.close();
                throw create_error;
            }
            state = {
                handle: handle,
                sequence: sequence,
                turn_first: chunk_turn_first,
                turn_last: chunk_turn_last,
                chunk_count: 0,
                bytes_written: header.byteLength,
            };
            open_files[tier] = state;
        }

        var length_prefix = new ArrayBuffer(4);
        new DataView(length_prefix).setUint32(0, chunk.byteLength, true);
        state.handle.write(new Uint8Array(length_prefix), { at: state.bytes_written });
        state.handle.write(chunk, { at: state.bytes_written + 4 });
        state.bytes_written += 4 + chunk.byteLength;
        state.chunk_count += 1;
        state.turn_last = chunk_turn_last;

        var patch = new ArrayBuffer(8);
        var patch_view = new DataView(patch);
        patch_view.setUint32(0, state.turn_last, true);
        patch_view.setUint32(4, state.chunk_count, true);
        state.handle.write(new Uint8Array(patch), { at: HISTORY_FILE_PATCH_OFFSET });
        state.handle.flush();

        if (state.bytes_written >= HISTORY_FILE_BYTES) {
            state.handle.close();
            open_files[tier] = null;
            post_file_closed(tier, state.sequence, state.turn_first, state.turn_last, state.bytes_written);
        }
    } catch (e) {
        self.postMessage({ type: "error" });
    }
}

async function preserve_previous_head_state() {
    var current_handle = null;
    var prev_handle = null;
    try {
        var current_fh = await dir_handle.getFileHandle(HISTORY_STATE_NAME);
        current_handle = await current_fh.createSyncAccessHandle();
        var size = current_handle.getSize();
        if (size === 0) {
            throw new Error("head.ths is 0 bytes");
        }
        var current_bytes = new Uint8Array(size);
        current_handle.read(current_bytes, { at: 0 });
        current_handle.close();
        current_handle = null;

        var prev_fh = await dir_handle.getFileHandle(HISTORY_STATE_NAME + HISTORY_STATE_PREV_SUFFIX, { create: true });
        prev_handle = await prev_fh.createSyncAccessHandle();
        prev_handle.truncate(0);
        prev_handle.write(current_bytes, { at: 0 });
        prev_handle.flush();
        prev_handle.close();
        prev_handle = null;
    } finally {
        if (current_handle !== null) current_handle.close();
        if (prev_handle !== null) prev_handle.close();
    }
}

async function write_head_state(bytes) {
    var handle = null;
    try {
        if (is_head_written_this_session || is_inherited_head_good) {
            await preserve_previous_head_state();
        }
        var fh = await dir_handle.getFileHandle(HISTORY_STATE_NAME, { create: true });
        handle = await fh.createSyncAccessHandle();
        handle.truncate(0);
        handle.write(new Uint8Array(bytes), { at: 0 });
        handle.flush();
        is_head_written_this_session = true;
    } catch (e) {
        self.postMessage({ type: "error" });
    } finally {
        if (handle !== null) handle.close();
    }
}

async function write_snapshots(bytes) {
    try {
        var data = new Uint8Array(bytes);
        await write_whole_file(dir_handle, HISTORY_SNAPSHOTS_NAME + HISTORY_STATE_PREV_SUFFIX, data);
        await write_whole_file(dir_handle, HISTORY_SNAPSHOTS_NAME, data);
        self.postMessage({ type: "snapshots_written" });
    } catch (e) {
        self.postMessage({ type: "error" });
    }
}
`;

    function init(params) {
        if (typeof Worker === "undefined" || typeof navigator.storage === "undefined") {
            ready_state = -1;
            return;
        }
        var blob = new Blob([worker_code], { type: "application/javascript" });
        var url = URL.createObjectURL(blob);
        worker = new Worker(url);
        URL.revokeObjectURL(url);
        refresh_storage_estimate();

        worker.onmessage = function (event) {
            var message = event.data;
            switch (message.type) {
                case "ready":
                    ready_state = message.value;
                    break;
                case "persistent":
                    persistent_state = message.value;
                    break;
                case "snapshots_written":
                    refresh_storage_estimate();
                    break;
                case "boot_probe":
                    boot_blob = new Uint8Array(message.bytes);
                    break;
                case "loaded":
                    if (load_slot !== null) {
                        ready_state = -1;
                    } else {
                        load_slot = { tier: message.tier, sequence: message.sequence, bytes: new Uint8Array(message.bytes) };
                    }
                    break;
                case "file_closed":
                    var record = new Uint8Array(closed_record_size);
                    var record_view = new DataView(record.buffer);
                    record_view.setUint32(0, message.tier, true);
                    record_view.setUint32(4, message.sequence, true);
                    record_view.setUint32(8, message.turn_first, true);
                    record_view.setUint32(12, message.turn_last, true);
                    record_view.setUint32(16, message.byte_length, true);
                    closed_queue.push(record);
                    break;
                case "error":
                    if (!is_resetting) {
                        ready_state = -1;
                    }
                    break;
                case "reset_done":
                    location.reload();
                    break;
            }
        };

        params.type = "init";
        worker.postMessage(params);
    }

    function register_plugin(importObject) {
        importObject.env.jam_ld60_old_tech_history_init = function (
            format_version,
            tier_count, file_bytes,
            ratio_0, ratio_1, ratio_2, ratio_3, ratio_4,
            magic_0, magic_1, magic_2, magic_3, magic_4, magic_5, magic_6, magic_7,
            state_magic_0, state_magic_1, state_magic_2, state_magic_3,
            state_magic_4, state_magic_5, state_magic_6, state_magic_7,
            manifest_name_ptr, manifest_name_len,
            sectors_name_ptr, sectors_name_len,
            state_name_ptr, state_name_len,
            levels_dir_name_ptr, levels_dir_name_len,
            file_ext_ptr, file_ext_len,
            file_header_bytes,
            closed_record_bytes,
            file_patch_offset,
            state_prev_suffix_ptr, state_prev_suffix_len,
            manifest_save_id_offset,
            file_tier_offset,
            file_turn_first_offset,
            chunk_turn_first_offset,
            chunk_turn_last_offset,
            manifest_size,
            snapshots_name_ptr, snapshots_name_len
        ) {
            var memory = wasm_memory.buffer;
            closed_record_size = closed_record_bytes >>> 0;
            init({
                format_version: format_version >>> 0,
                tier_count: tier_count >>> 0,
                file_bytes: file_bytes >>> 0,
                tier_ratio: [ratio_0 >>> 0, ratio_1 >>> 0, ratio_2 >>> 0, ratio_3 >>> 0, ratio_4 >>> 0],
                file_magic: [magic_0 & 0xff, magic_1 & 0xff, magic_2 & 0xff, magic_3 & 0xff,
                             magic_4 & 0xff, magic_5 & 0xff, magic_6 & 0xff, magic_7 & 0xff],
                state_magic: [state_magic_0 & 0xff, state_magic_1 & 0xff, state_magic_2 & 0xff, state_magic_3 & 0xff,
                              state_magic_4 & 0xff, state_magic_5 & 0xff, state_magic_6 & 0xff, state_magic_7 & 0xff],
                manifest_name: name_decoder.decode(new Uint8Array(memory, manifest_name_ptr, manifest_name_len)),
                sectors_name: name_decoder.decode(new Uint8Array(memory, sectors_name_ptr, sectors_name_len)),
                state_name: name_decoder.decode(new Uint8Array(memory, state_name_ptr, state_name_len)),
                levels_dir_name: name_decoder.decode(new Uint8Array(memory, levels_dir_name_ptr, levels_dir_name_len)),
                file_ext: name_decoder.decode(new Uint8Array(memory, file_ext_ptr, file_ext_len)),
                file_header_bytes: file_header_bytes >>> 0,
                file_patch_offset: file_patch_offset >>> 0,
                state_prev_suffix: name_decoder.decode(new Uint8Array(memory, state_prev_suffix_ptr, state_prev_suffix_len)),
                manifest_save_id_offset: manifest_save_id_offset >>> 0,
                file_tier_offset: file_tier_offset >>> 0,
                file_turn_first_offset: file_turn_first_offset >>> 0,
                chunk_turn_first_offset: chunk_turn_first_offset >>> 0,
                chunk_turn_last_offset: chunk_turn_last_offset >>> 0,
                manifest_size: manifest_size >>> 0,
                snapshots_name: name_decoder.decode(new Uint8Array(memory, snapshots_name_ptr, snapshots_name_len))
            });
        };
        importObject.env.jam_ld60_old_tech_history_ready = function () {
            return ready_state;
        };
        importObject.env.jam_ld60_old_tech_history_clear_failed_save = function () {
            if (worker === null) {
                ready_state = -2;
                return;
            }
            ready_state = 0;
            boot_blob = null;
            worker.postMessage({ type: "clear_failed_save" });
        };
        importObject.env.jam_ld60_old_tech_history_reset_game = function () {
            if (worker === null) return;
            is_resetting = true;
            worker.postMessage({ type: "reset_game" });
        };
        importObject.env.jam_ld60_old_tech_history_persistent = function () {
            return persistent_state;
        };
        importObject.env.jam_ld60_old_tech_history_write = function (tier, ptr, len) {
            if (worker === null) return;
            var bytes = new Uint8Array(wasm_memory.buffer, ptr, len).slice();
            worker.postMessage({ type: "write_chunk", tier: tier, bytes: bytes.buffer }, [bytes.buffer]);
        };
        importObject.env.jam_ld60_old_tech_history_write_head_state = function (ptr, len) {
            if (worker === null) return;
            var bytes = new Uint8Array(wasm_memory.buffer, ptr, len).slice();
            worker.postMessage({ type: "write_head_state", bytes: bytes.buffer }, [bytes.buffer]);
        };
        importObject.env.jam_ld60_old_tech_history_write_snapshots = function (ptr, len) {
            if (worker === null) return;
            var bytes = new Uint8Array(wasm_memory.buffer, ptr, len).slice();
            worker.postMessage({ type: "write_snapshots", bytes: bytes.buffer }, [bytes.buffer]);
        };
        importObject.env.jam_ld60_old_tech_history_storage_free = function () {
            return storage_free;
        };
        importObject.env.jam_ld60_old_tech_history_mark_head_good = function () {
            if (worker === null) return;
            worker.postMessage({ type: "mark_head_good" });
        };
        importObject.env.jam_ld60_old_tech_history_write_bundle = function (ptr, len) {
            if (worker === null) return;
            var bytes = new Uint8Array(wasm_memory.buffer, ptr, len).slice();
            worker.postMessage({ type: "write_bundle", bytes: bytes.buffer }, [bytes.buffer]);
        };
        importObject.env.jam_ld60_old_tech_history_seed_sequences = function (ptr, count) {
            if (worker === null) return;
            var view = new DataView(wasm_memory.buffer, ptr, count * 4);
            var seeds = new Array(count);
            for (var i = 0; i < count; i++) {
                seeds[i] = view.getUint32(i * 4, true);
            }
            worker.postMessage({ type: "seed_sequences", seeds: seeds });
        };
        importObject.env.jam_ld60_old_tech_history_boot_len = function () {
            return boot_blob === null ? 0 : boot_blob.length;
        };
        importObject.env.jam_ld60_old_tech_history_boot_copy = function (ptr) {
            if (boot_blob === null) return;
            new Uint8Array(wasm_memory.buffer).set(boot_blob, ptr);
            boot_blob = null;
        };
        importObject.env.jam_ld60_old_tech_history_commit_fresh = function (
            manifest_ptr, manifest_len, sectors_ptr, sectors_len, bundle_ptr, bundle_len, head_ptr, head_len
        ) {
            if (worker === null) return;
            var manifest = new Uint8Array(wasm_memory.buffer, manifest_ptr, manifest_len).slice();
            var sectors = new Uint8Array(wasm_memory.buffer, sectors_ptr, sectors_len).slice();
            var bundle = new Uint8Array(wasm_memory.buffer, bundle_ptr, bundle_len).slice();
            var head = new Uint8Array(wasm_memory.buffer, head_ptr, head_len).slice();
            worker.postMessage({
                type: "commit_fresh",
                manifest_bytes: manifest.buffer,
                sectors_bytes: sectors.buffer,
                bundle_bytes: bundle.buffer,
                head_bytes: head.buffer
            }, [manifest.buffer, sectors.buffer, bundle.buffer, head.buffer]);
        };
        importObject.env.jam_ld60_old_tech_history_request_load = function (tier, turn_first, sequence) {
            if (worker === null) return;
            worker.postMessage({ type: "request_load", tier: tier, turn_first: turn_first >>> 0, sequence: sequence >>> 0 });
        };
        importObject.env.jam_ld60_old_tech_history_load_state = function () {
            return load_slot === null ? 0 : 1;
        };
        importObject.env.jam_ld60_old_tech_history_load_tier = function () {
            return load_slot === null ? 0 : load_slot.tier;
        };
        importObject.env.jam_ld60_old_tech_history_load_sequence = function () {
            return load_slot === null ? 0 : load_slot.sequence;
        };
        importObject.env.jam_ld60_old_tech_history_load_len = function () {
            return load_slot === null ? 0 : load_slot.bytes.length;
        };
        importObject.env.jam_ld60_old_tech_history_load_copy = function (ptr) {
            if (load_slot === null) return;
            new Uint8Array(wasm_memory.buffer).set(load_slot.bytes, ptr);
            load_slot = null;
        };
        importObject.env.jam_ld60_old_tech_history_delete = function (tier, turn_first, sequence) {
            if (worker === null) return;
            worker.postMessage({ type: "delete_file", tier: tier, turn_first: turn_first >>> 0, sequence: sequence >>> 0 });
        };
        importObject.env.jam_ld60_old_tech_history_republish = function (tier, ptr, len) {
            if (worker === null) return;
            var bytes = new Uint8Array(wasm_memory.buffer, ptr, len).slice();
            worker.postMessage({ type: "republish", tier: tier, bytes: bytes.buffer }, [bytes.buffer]);
        };
        importObject.env.jam_ld60_old_tech_history_closed_next = function (ptr) {
            if (closed_queue.length === 0) return 0;
            var record = closed_queue.shift();
            new Uint8Array(wasm_memory.buffer).set(record, ptr);
            return 1;
        };
    }

    miniquad_add_plugin({
        register_plugin: register_plugin,
        version: 1,
        name: "jam_ld60_old_tech_history"
    });
})();
