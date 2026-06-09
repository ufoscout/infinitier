# Keeper

## ToDo List

- [ ] EngineCaps should be part of the GameData
- [ ] Keeper Innate tab should show spell icons
- [ ] Keeper Wizard tab should show spell icons
- [ ] Keeper Priest tab should show spell icons


## Check Game values

- [X] bg
- [X] bgee
    - [X] Characteristics -> Miscellaneous issues
    - [X] Proficiencies -> Wrong for dual-classed characters (e.g Imoen)
- [X] bg2
    - [X] Characteristics -> Palgorn should have kit "Inquisitor" but has nothing
    - [X] Characteristics -> Valygar should have kit "Stalker" but has nothing
- [X] bg2ee
    - [X] Characteristics -> Miscellaneous issues
    - [X] Proficiencies -> Check Imoen for a dual class
    - [X] Proficiencies -> Create a dual classed character and check the values
- [ ] iwd (TODO)
- [X] iwdee
    - [X] Proficiencies -> Create a dual classed character and check the values
    - [X] Characteristics -> Fix Kit and Racial labels
- [ ] iwd2 (TODO)
- [ ] pst (TODO)
    - [ ] Inventory -> missing
- [X] pstee
    - [X] Abilities -> Thief skills bonus are slightly different from EEkeeper (We are right here, EEkeeper is wrong)
    - [X] Characteristics -> Wrong kits for some classes
    - [X] Characteristics -> Original class for some dual-classed characters not reported (ignore it, I think we are right here)
    - [X] Characteristics -> Miscellaneous issues
    - [X] Inventory -> missing
    - [X] Proficiency -> wrong list and values
    - [X] Resistances -> Ignus -> cold & magic cold are reported as 206 but it should be -50
    - [X] Miscellaneous -> Others -> Tracking Target is empty in EEkeeper but has strange values in Keeper
    - [X] Different number of attacks with the game -> due to the race and some hard-coded values (e.g. Morte has 3 attacks)
    - [X] Morte has 100% poison resistance in the game: this is nowhere in the savegame


## Keeper errors:
- [ ] Tables are different in different tabs
- [ ] Table rows are not clickable where there is text
- [ ] should load a game with a double click
- [X] Check why the 16th Inventory slot is called "Magic Weapon"
      - In Sword Keeper it is called "Inventory"
      - In Shadow Keeper it is called "Inventory"


