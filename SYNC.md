# Typo sync format

Typo keeps a notes folder the same on every device through an object store that you own. The
desktop app and the Android app use the format in this file. A third app that follows it can
read and write the same notes.

The format is Quire's, unchanged: Typo was Quire until 2.0.0. The names in it, such as `.quire/`,
`QUIRE1`, `%QUIRE-LOCKED-NOTE` and the default prefix `quire/`, keep Quire's name, so a folder that
Quire synced syncs on in Typo.

The reference code is in `desktop/src/sync/`. The tests in `desktop/src/sync/tests.rs` run two
devices against one bucket.

## Remote folder

All objects of one notes folder sit under one key prefix. The default prefix is
`quire/<folder name>/`. You can change it in the **Remote folder** field. The prefix always ends
in `/`, unless it is empty.

Each notes folder on a device has its own prefix. Two notes folders with one prefix merge their
notes, so an app must not use one prefix value for all its folders. To sync one notes folder
between two devices, give it the same prefix on both.

| Key under the prefix | Content |
| --- | --- |
| `.quire/format.json` | Plain JSON. It tells if the notes are encrypted, and holds the key salt. |
| `.quire/manifest.json` | The manifest as plain JSON, for a folder that is not encrypted. |
| `.quire/manifest.bin` | The manifest as a sealed blob, for an encrypted folder. |
| `<path>` | The text of one note or the bytes of one image, for a folder that is not encrypted. Example: `Drafts/Plan.md`. |
| `objects/<name>.bin` | The sealed text of one note or the sealed bytes of one image, for an encrypted folder. |

A note path is relative to the notes folder and uses `/` between folders. Notes are the files
that end in `.md`. Images sync too: the files that end in `.png`, `.jpg`, `.jpeg`, `.gif`,
`.webp`, `.bmp`, `.svg` or `.avif`, in any letter case, up to 50 MB each. No other file syncs.
Files and folders whose names start with `.` do not sync, and neither does `node_modules`. Line endings are `\n`. An app converts `\r\n` to `\n` when it reads a note.

## Format file

```json
{
  "version": 1,
  "encrypted": true,
  "kdf": "pbkdf2-sha256",
  "iterations": 600000,
  "salt": "<16 random bytes, standard base64>",
  "check": "<the sealed bytes of the ASCII text quire, standard base64>"
}
```

A folder that is not encrypted has only `version` and `encrypted`. The first device that syncs
writes this file. Every later device reads it first and stops with an error in these cases:

- The file says `encrypted` and the device has encryption off.
- The file does not say `encrypted` and the device has encryption on.
- The passphrase does not open `check`.

## Manifest

```json
{
  "version": 1,
  "files": {
    "Drafts/Plan.md": {
      "hash": "<SHA-256 of the note text as UTF-8, or of the image bytes, lowercase hex>",
      "object": "<the key of the note object, under the prefix>",
      "time": 1762941600000,
      "device": "laptop"
    }
  },
  "folders": ["Archive", "Drafts"],
  "pins": ["Drafts/Plan.md"]
}
```

`folders` lists every folder in the notes folder, empty folders included. A manifest without
`folders` has no folders. `pins` lists the pinned notes. A manifest without `pins` comes from a
version without pins. Such a manifest says nothing about pins, so a device keeps its own pins
and writes them. `time` is the upload time in milliseconds since 1970. `device` is the
name of the device that uploaded the file. The desktop app uses the host name. The Android app
uses the device model.

## Encryption

1. Derive 64 bytes with PBKDF2-HMAC-SHA256 from the passphrase as UTF-8, the salt and the
   iteration count in the format file.
2. Use bytes 0 to 31 as the AES-256-GCM key.
3. Use bytes 32 to 63 as the name key.

A sealed blob is the ASCII bytes `QUIRE1`, then a random 12-byte nonce, then the AES-GCM
ciphertext with its 16-byte tag at the end. There is no associated data.

The object name of a note is `objects/` plus the first 40 hex characters of
HMAC-SHA256 with the name key over the note path as UTF-8, plus `.bin`. Thus the store sees
neither note names nor note text. The passphrase needs at least 12 characters.

## One sync

A device keeps a base record for each notes folder and store. The base maps each note path to
the hash that both sides had after the last sync on this device. It also lists the folders that
both sides had. Its JSON form is:

```json
{"files": {"<path>": "<hash>"}, "folders": ["<path>"], "stamps": {"<image path>": [size, time, "<hash>"]}, "pins": ["<path>"]}
```

`pins` lists the pins after the last sync. A base without `pins` comes from before pins synced.
`stamps` is optional. It holds the size, the file time in nanoseconds and the hash of each
image, so an image that did not change is not read again. The desktop app keeps the base in
`sync/` in its config folder, which is `~/.config/typo/sync/` on Linux. The base never goes to
the store.

For each path in the local folder, the manifest or the base, compare the local hash `L`, the
remote hash `R` and the base hash `B`:

| Local | Remote | Condition | Action |
| --- | --- | --- | --- |
| yes | yes | `L = R` | Nothing. The note is in step. |
| yes | no | `L = B` | Another device deleted the note. Move it to `.trash`. |
| yes | no | `L ≠ B` | Upload the note. It is new, or it changed after the remote delete. |
| no | yes | `R = B` | This device deleted the note. Delete the object and the manifest entry. |
| no | yes | `R ≠ B` | Download the note. |
| yes | yes | `B = R` | Only this device changed the note. Upload it. |
| yes | yes | `B = L` | Only the other device changed the note. Download it. |
| yes | yes | other | Both changed the note. See the next section. |

Images use the same table. A manifest entry for an image looks like one for a note.

A path with a `..` part, an empty part or a part that starts with `.` is skipped, and so is a path
that is neither a note nor an image.

Then compare the folders in the same way. A folder is kept when a note or an image is still in
it after the file steps. Such a file stayed, came down or went up in this sync.

| Local | Remote | Condition | Action |
| --- | --- | --- | --- |
| yes | yes | none | Nothing. |
| yes | no | in the base, no note kept in it | Another device deleted the folder. Move it to `.trash`. |
| yes | no | other | Add the folder to the manifest. |
| no | yes | in the base, no note kept in it | This device deleted the folder. Remove it from the manifest. |
| no | yes | other | Make the folder. |

Move the deepest folders to `.trash` first. A folder moves with every file in it, also files that
are not notes.

Then merge the pins. Pins sync for each notes folder, in the manifest of its prefix.

1. If the manifest has no `pins`, the result is the local pins.
2. If the base has no `pins`, the result is the local pins plus the remote pins. Nothing is
   removed in the first sync with pins.
3. Otherwise, a pin on both sides stays. A pin on one side only stays if the base does not have
   it, because the other side added it. If the base has it, the other side removed it, so it goes.
4. Remove from the result each pin whose note is not kept after the file steps.

If the result differs from the remote pins, add the pins that are new and remove the pins that
are gone. A manifest without `pins` gets `pins` only when the result is not empty. Save the result
in the base. The app takes the pins that the sync added or removed, compared with the pins it sent.
A pin that changed on the device during the sync stays changed.

After these steps, read the manifest again, apply the changes of this sync to it, and write it.
The second read keeps the entries that another device wrote during the sync.

Save the new base only after the downloads, the deletes and the folder changes are on disk. It
holds each path that is in step after this sync. If a local write fails, delete the base. A base
that says a file is in step when the old version is still on disk makes the next sync upload
the old version over the newer remote one. Without a base, the next sync keeps both versions of
each file that differs.

## Conflicts

When both sides changed a note, the device keeps its own text at the path and uploads it. The
remote text goes to a new note beside it, named `<name> (conflict <device>).md`. An image follows
the same rule and keeps its extension, as in `cat (conflict laptop).png`. The device
uploads that copy at once, so every device gets both versions. If the name is taken, the device
adds a number: `<name> (conflict <device> 2).md`.

An app must not overwrite a file that changed while a sync ran. Before it writes a downloaded
note or image, it compares the current file with the hash the sync saw. If the two differ, it
writes the download as a conflict copy. A delete from another device applies only to a file
that did not change.

## Stores

All S3 requests use AWS Signature Version 4 with the service `s3`. Each request signs the
`host`, `x-amz-content-sha256` and `x-amz-date` headers. The payload hash is the SHA-256 of the
body. A GET that returns 404 with the code `NoSuchKey` means that the object does not exist.

| Provider id | Endpoint | Signing region | Addressing |
| --- | --- | --- | --- |
| `s3` | `https://s3.<region>.amazonaws.com` | the region | virtual-hosted |
| `hetzner` | `https://<region>.your-objectstorage.com` | the region | path-style |
| `r2` | `https://<accountId>.r2.cloudflarestorage.com` | `auto` | path-style |
| `b2` | `https://s3.<region>.backblazeb2.com` | the region | path-style |
| `do` | `https://<region>.digitaloceanspaces.com` | `us-east-1` | path-style |
| `wasabi` | `https://s3.<region>.wasabisys.com` | the region | path-style |
| `scaleway` | `https://s3.<region>.scw.cloud` | the region | path-style |
| `ovh` | `https://s3.<region>.io.cloud.ovh.net` | the region | path-style |
| `custom` | the `endpoint` field | the `region` field | the `addressing` field |
| `gcs` with HMAC keys | `https://storage.googleapis.com` | `auto` | path-style |

The provider fields are `bucket`, `accessKey`, `secretKey` and `region`. R2 also has
`accountId`. The prefix is not a provider field: the settings keep one prefix for each notes
folder, in `folders`. A custom server also has `endpoint` and `addressing`, which is `path` or `vhost`.
Without a picked region, an app uses the first region that the settings list for the provider.

**Google Cloud Storage with a service account key** (`gcsAuth` is `sa`):

1. Sign a JWT with RS256 and the `private_key` from the key file in `gcsJson`. The claims are
   `iss` set to `client_email`, `scope` set to
   `https://www.googleapis.com/auth/devstorage.read_write`, `aud` set to `token_uri`, `iat`, and
   `exp` set to `iat` plus 3600.
2. Post it to `token_uri` as the `assertion` of the grant type
   `urn:ietf:params:oauth:grant-type:jwt-bearer`.
3. Send the access token as `Authorization: Bearer <token>` to the JSON API:
   - Get an object: `GET /storage/v1/b/<bucket>/o/<key>?alt=media`
   - Put an object: `POST /upload/storage/v1/b/<bucket>/o?uploadType=media&name=<key>`
   - Delete an object: `DELETE /storage/v1/b/<bucket>/o/<key>`

The key in these paths is fully URL-encoded, `/` included. The fields for HMAC keys are `hmacId`
and `hmacSecret`.

**Azure Blob Storage** uses `https://<account>.blob.core.windows.net/<container>/<key>` with
`x-ms-version: 2021-08-06`. A PUT sends `x-ms-blob-type: BlockBlob`. The fields are `account`,
`container`, `azAuth` and `azKey`. With `azAuth` set to `key`, `azKey` is the base64 account
key, and each request carries a Shared Key signature. With `azAuth` set to `sas`, `azKey` is a
SAS token that the app appends to the URL as the query string. The token needs read, write,
create and delete permissions on the container.

## Requests

A device tries a request again up to 4 times, after 0.5, 1, 2 and 4 seconds, when the answer is
429, 500, 502, 503 or 504, or when the connection fails. It does not try again after a DNS error
or a URL that is not valid. Each try signs the request again. Downloads, uploads and deletes run
8 at a time. The manifest is written after all of them.

## When a device syncs

- When a notes folder opens. On Android, also when the app comes to the front and when it goes
  to the background.
- After a save, a delete, a rename, a move or a new folder, when **When to sync** is **On save**.
  The sync starts 1.5 seconds after the change.
- Every 5 minutes, when **When to sync** is **Every 5 min**.
- When you select **Sync now** in the settings or in the command palette.

A device syncs only after **Test connection** passed with the current settings. When
**When to sync** is **Manually**, only **Sync now** starts a sync.

## Locked notes

A note can have its own password, apart from the sync passphrase. A locked note is still a
`.md` file, and it syncs like any other note. Its text is:

```text
%QUIRE-LOCKED-NOTE v1 pbkdf2-sha256 <iterations> <salt, standard base64>

This note is locked with a password. Open it in Typo to read it.

<the sealed text, standard base64, in lines of 76 characters>
```

1. Derive 64 bytes with PBKDF2-HMAC-SHA256 from the password as UTF-8, the salt and the
   iteration count in the first line. Typo uses 16 random bytes of salt and 600000 iterations.
2. Use bytes 0 to 31 as the AES-256-GCM key.
3. The sealed text is a sealed blob, as in "Encryption": `QUIRE1`, a 12-byte nonce, then the
   ciphertext of the note text as UTF-8 with its tag.

The notice line is for other apps, and its wording can change: Quire wrote "Open it in Quire".
To read the body, skip the first line, the blank lines and the notice line (the only line with
spaces), and join the rest without line breaks. Each save makes a new nonce and keeps the salt,
so the file changes even when the text does not. A note whose first
line starts with `%QUIRE-LOCKED-NOTE` and has a version other than `v1` must stay locked.

An app keeps the plain text of a locked note in memory only. Search, the link index and the tag
list skip locked notes. The password needs at least 8 characters.

## Test vectors

These values come from `cargo test print_vectors -- --nocapture` in `desktop/`. A second
AES-GCM implementation opens both sealed blobs.

| Input | Value |
| --- | --- |
| Passphrase | `correct horse battery staple` |
| Salt | `0123456789abcdef` as ASCII, base64 `MDEyMzQ1Njc4OWFiY2RlZg==` |
| Iterations | `1000` |
| AES key, hex | `caa4aad92ca0635b01e04707f5fd851b42533044bec2a2ec393e589a2441a652` |
| Name key, hex | `d86dac01e25c2374f088dcdb35777a514fdffe3c66b5b055cdf2ac23671f52f2` |
| Object name of `Drafts/Plan.md` | `objects/106404d37208edd67089d2ad1258f2d0385c19fa.bin` |
| Sealed `# Blåbær`, base64 | `UVVJUkUxyHeerIFLzO6WIrgd6vd6nFBCHkNaN/NGe60nmwng72y4MbVhouA=` |
| Sealed `quire`, base64 | `UVVJUkUx1R27W/XnzAUMDiCCWoIsHIQv6CJ8Dn8sKqPUlrdkcpwZ` |
| SHA-256 of `# Plan` | `1ee55b7ae632bd99dfea4f2d791ff06b1119975e96d4f822050923e7499f44cb` |

This locked note opens with the password `tiger lily` to the text `# Diary\n\nBlåbær\n`. It
comes from `cargo test print_locked_note -- --nocapture`:

```text
%QUIRE-LOCKED-NOTE v1 pbkdf2-sha256 1000 MDEyMzQ1Njc4OWFiY2RlZg==

This note is locked with a password. Open it in Typo to read it.

UVVJUkUxuYsECSutmGY8MVrT44GXP+mqVNaRrkyLtnVcXqlJUQ/+AdtwaCyZUuSDFLdqVw==
```

The AWS documentation example for a GET Object request also applies to SigV4. Its signature is
`f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41`.
