# Rotating the vault's key

`vault-backup.sh` encrypts each snapshot to the public key the data
repository's `VAULT_RECIPIENT` names; the private half is the restore key,
kept off the box in the owner's password manager. Rotating it is the one way
to make every snapshot taken so far unreadable at once: pruning removes a
snapshot after seven daily runs, but a copy of one taken before an erasure
can always restore what it held, until the key that opens it is gone.

Each step is the owner's, by hand. Nothing here runs unattended, and no
secret is pasted into a chat or a session.

1. **Make the new key on a trusted machine, never the box.** The box needs
   only the public half to encrypt. A box restored by `bootstrap-box.sh`
   holds the old private half as well, since the restore imports it; step 5
   removes it there too.

   ```sh
   uid="suzatary vault $(date -u +%Y-%m)"
   gpg --batch --quick-generate-key "$uid" future-default default never
   gpg --list-keys --with-colons "=$uid" | awk -F: '/^fpr/ {print $10; exit}'
   ```

   The last line prints the new primary key's fingerprint (`=` matches the
   UID exactly, and only the first fingerprint is printed), `NEW` below;
   the old one is the one `VAULT_RECIPIENT` names now, `OLD`.

2. **Keep the new restore key.**

   ```sh
   gpg --armor --export-secret-keys NEW > vault-restore-key.asc
   gpg --armor --export NEW > vault-public.asc
   ```

   Put `vault-restore-key.asc` in the password manager beside the old one
   (both, until step 5), and delete the file from disk.

3. **Point the box at it.** Copy `vault-public.asc` to the box, then:

   ```sh
   gpg --import vault-public.asc
   echo NEW > "$(dirname "$(realpath "$SUZATARY_INSTANCE")")/VAULT_RECIPIENT"
   ```

   and commit `VAULT_RECIPIENT` in the data repository.

4. **Take a snapshot and restore it.** Run
   `systemctl --user start suzatary-vault.service`, download the newest
   `<box>-vault-*.tar.gz.gpg` on the trusted machine, and check that
   `gpg -d <it> | tar tzf -` lists the vault's contents with the new key.
   Do not go on until it does: a rotation that cannot restore is a lost
   vault.

5. **Destroy the old key.** On every machine that holds it, the box
   included:

   ```sh
   gpg --delete-secret-and-public-keys OLD
   gpg --list-secret-keys OLD   # must answer: no secret key
   ```

   On a box restored by `bootstrap-box.sh`, the bootstrap's own keyring
   holds the old private key too: the restore imported it before moving that
   keyring aside as `~/.gnupg.bootstrap`. Check what it holds with `gpg
   --homedir ~/.gnupg.bootstrap --list-secret-keys` (it is the disposable
   keyring the bootstrap made, with `OLD` in it), then remove the whole
   directory: `rm -rf ~/.gnupg.bootstrap`. Remove every
   `vault-restore-key.asc` that holds `OLD`, and delete the password
   manager's item for it, its history included, since a password manager
   keeps earlier versions of an item. From this moment no snapshot
   encrypted to `OLD` opens, wherever a copy of one sits, and the next
   seven runs prune them from the Drive too.

`bootstrap-box.sh` needs no change: it is handed the restore key file and
restores the newest snapshot, which is under `NEW` from step 4 on.
