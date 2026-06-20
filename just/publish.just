
# Dry-run publish for every publishable crate (no upload, verifies
# packaging).
publish-dry-run:
    cargo publish --workspace --dry-run

# Publish every publishable crate to crates.io, one at a time.
publish gap="5":
    #!/usr/bin/env bash

    # We can't use `cargo publish` because it hits 429 (too many crates) error

    set -euo pipefail
    crates=(
        infinitier_datasource
        infinitier_common
        infinitier_fs
        infinitier_acm_decoder
        infinitier_bik_decoder
        infinitier_mve_decoder
        infinitier_wbm_decoder
        infinitier_mve_encoder
        infinitier_acm_encoder
        infinitier_wav_resource
        infinitier_acm_resource
        infinitier_pvrz_resource
        infinitier_bam_resource
        infinitier_ids_resource
        infinitier_bcs_resource
        infinitier_bif_resource
        infinitier_bmp_resource
        infinitier_cre_resource
        infinitier_fnt_resource
        infinitier_gam_resource
        infinitier_ini_resource
        infinitier_itm_resource
        infinitier_key_resource
        infinitier_mos_resource
        infinitier_png_resource
        infinitier_sav_resource
        infinitier_spl_resource
        infinitier_tis_resource
        infinitier_tlk_resource
        infinitier_ttf_resource
        infinitier_two_da_resource
        infinitier_wed_resource
        infinitier_core
    )
    count=${#crates[@]}
    for i in "${!crates[@]}"; do
        c="${crates[$i]}"
        echo "==> [$((i + 1))/$count] publishing $c"
        if out=$(cargo publish -p "$c" 2>&1); then
            echo "$out"
        else
            echo "$out"
            if echo "$out" | grep -qiE "already (uploaded|exists)"; then
                echo "    (this version is already published — skipping)"
            else
                echo "!! publish of $c failed" >&2
                exit 1
            fi
        fi
        # Pause before the next crate (not after the last one).
        if [ "$i" -lt "$((count - 1))" ]; then
            echo "    waiting {{gap}}s before the next crate..."
            sleep {{gap}}
        fi
    done
    echo "All crates published."
