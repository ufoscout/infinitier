# infinitier_core



## Unhardcoded game resources

The unhardcoded game resources in this folder contains a set of GemRB-supplied game resources — .2da tables, .spl spells, .pro projectiles, .vvc visual effects, .chu UI layouts, .ids and .ini files — organized per game (bg1, bg2, bgee, bg2ee, iwd, how, iwd2, pst) plus a shared/ folder common to all.
This data is hardcoded in the original Infinity Engine, the GemRB engine unhardcoded them.

We include them in our core with lowest priority so they can be overriden by the user.

There is `just` command to update them from the GemRB github repo.