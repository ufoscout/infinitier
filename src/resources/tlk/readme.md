# infinitier_tlk_resource

Read-only loader for IE-engine TLK string tables (`dialog.tlk` /
`dialogF.tlk`). Minimal MVP — exposes one helper, `Tlk::get(strref)`,
that returns the localized string at a given strref. Sound-slot
metadata (variance, pitch, attached resref) is preserved verbatim
but not exposed yet.

Only TLK V1 is supported (all shipped IE games use V1; the V2 spec is
a community draft that no engine recognises).
