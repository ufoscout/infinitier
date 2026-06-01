# Keeper

## ToDo List

- [ ] EngineCaps should be part of the GameData



### Fix Lore and MaxHP calculations:

Character    Lore(Game)   MaxHP(Game)   Lore(Keeper)   MaxHP_effective(Keeper)
------------------------------------------------------------------------------
Xor                  87           217             32                       227
Minsc                 6           192             36                       228
Keldorn              35           160             30                       193
Nalia                86            82             84                       118
Imoen                72            78             66                       102
Aerie                81            69             57                        69

It seems that both Lore and MaxHp depend on the class:
- lore: see https://baldursgate.fandom.com/wiki/Lore
- Max HP: search on Google

To be investigated if these values are per game, are hardcoded and/or are somewhere in the resources