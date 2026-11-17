use std::env;
use std::fs;
use std::process;

use dynamic_markdown_dock_lib::crypto::aead::{
    decrypt_blob, unwrap_keyfile_with_password, unwrap_keyfile_with_recovery,
};
use dynamic_markdown_dock_lib::crypto::vault_format::VaultHeader;

fn print_usage() {
    eprintln!(
        "Usage:\n  dmd-vault inspect <file.vault>\n  dmd-vault decrypt-blob <keyfile.vault> <blob.vault> <file_id_hex> <sha256_hex> [--password <pw> | --recovery <key>] [-o <out.md>]"
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage();
        process::exit(2);
    }

    match args[1].as_str() {
        "inspect" => {
            let bytes = fs::read(&args[2]).unwrap_or_else(|e| {
                eprintln!("Failed to read '{}': {e}", args[2]);
                process::exit(1);
            });
            match VaultHeader::parse(&bytes) {
                Ok((h, _, body)) => {
                    println!("Magic:       DMDV");
                    println!("Version:     {}", h.version);
                    println!("Kind:        {:?}", h.kind);
                    println!("AEAD:        {:?}", h.aead);
                    println!("Compression: {:?}", h.compression);
                    println!("Flags:       {:#06x}", h.flags);
                    println!("Header len:  {} bytes", h.header_len);
                    println!("Key epoch:   {}", h.key_epoch);
                    println!("Body len:    {} bytes", body.len());
                    if let Some(kdf) = h.kdf {
                        println!(
                            "KDF:         Argon2id (m={} KiB, t={}, p={})",
                            kdf.m_cost_kib, kdf.t_cost, kdf.p_cost
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Invalid .vault file: {e}");
                    process::exit(1);
                }
            }
        }
        "decrypt-blob" => {
            if args.len() < 6 {
                print_usage();
                process::exit(2);
            }
            let kf_bytes = fs::read(&args[2]).expect("failed to read keyfile");
            let blob_bytes = fs::read(&args[3]).expect("failed to read blob");
            let file_id: [u8; 32] = hex::decode(&args[4])
                .expect("invalid file_id hex")
                .try_into()
                .expect("file_id must be 32 bytes");
            let sha256: [u8; 32] = hex::decode(&args[5])
                .expect("invalid sha256 hex")
                .try_into()
                .expect("sha256 must be 32 bytes");

            let mut password: Option<String> = None;
            let mut recovery: Option<String> = None;
            let mut out_path: Option<String> = None;

            let mut i = 6;
            while i < args.len() {
                match args[i].as_str() {
                    "--password" if i + 1 < args.len() => {
                        password = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "--recovery" if i + 1 < args.len() => {
                        recovery = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "-o" if i + 1 < args.len() => {
                        out_path = Some(args[i + 1].clone());
                        i += 2;
                    }
                    other => {
                        eprintln!("Unknown argument: {other}");
                        process::exit(2);
                    }
                }
            }

            let (keys, _) = if let Some(pw) = password {
                unwrap_keyfile_with_password(&kf_bytes, &pw).expect("password unwrap failed")
            } else if let Some(rk) = recovery {
                unwrap_keyfile_with_recovery(&kf_bytes, &rk).expect("recovery unwrap failed")
            } else {
                eprintln!("Provide either --password or --recovery");
                process::exit(2);
            };

            let pt = decrypt_blob(&keys, &file_id, &sha256, &blob_bytes)
                .expect("blob decryption failed");
            if let Some(out) = out_path {
                fs::write(&out, &pt).expect("failed to write output file");
                println!("Decrypted {} bytes -> {out}", pt.len());
            } else {
                println!("{}", String::from_utf8_lossy(&pt));
            }
        }
        _ => {
            print_usage();
            process::exit(2);
        }
    }
}
