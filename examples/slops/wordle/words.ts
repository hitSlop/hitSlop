export type TileState = "correct" | "present" | "absent";

export const TARGET_WORDS: string[] = [
  "ABOVE", "ACUTE", "ADAPT", "ADMIT", "ADOPT", "ADULT", "AFTER", "AGAIN", "AGENT", "AGREE",
  "AHEAD", "ALARM", "ALBUM", "ALERT", "ALIEN", "ALIKE", "ALIVE", "ALLOW", "ALONE", "ALONG",
  "ALTER", "AMONG", "ANGEL", "ANGER", "ANGLE", "ANGRY", "APART", "APPLE", "APPLY", "ARENA",
  "ARGUE", "ARISE", "ARMOR", "ARRAY", "ARROW", "ASIDE", "ASSET", "AUDIO", "AUDIT", "AVOID",
  "AWAIT", "AWAKE", "AWARD", "AWARE", "BADGE", "BAKER", "BASIC", "BASIS", "BEACH", "BEARD",
  "BEAST", "BEGIN", "BEING", "BELOW", "BENCH", "BIRTH", "BLACK", "BLADE", "BLAME", "BLANK",
  "BLAST", "BLAZE", "BLEED", "BLEND", "BLESS", "BLIND", "BLOCK", "BLOOD", "BLOOM", "BLOWN",
  "BOARD", "BOAST", "BONUS", "BOOST", "BOOTH", "BOUND", "BRAIN", "BRAKE", "BRAND", "BRASS",
  "BRAVE", "BREAD", "BREAK", "BREED", "BRIEF", "BRING", "BRISK", "BROAD", "BROKE", "BROWN",
  "BRUSH", "BUDDY", "BUILD", "BUILT", "BUNCH", "BURST", "CABIN", "CABLE", "CANDY", "CANAL",
  "CARGO", "CARRY", "CARVE", "CATCH", "CAUSE", "CHAIN", "CHAIR", "CHALK", "CHAMP", "CHANCE",
  "CHANT", "CHAOS", "CHARM", "CHART", "CHASE", "CHEAP", "CHECK", "CHEEK", "CHEER", "CHEST",
  "CHIEF", "CHILD", "CHILL", "CHIPS", "CHORD", "CHUNK", "CIVIC", "CIVIL", "CLAIM", "CLAMP",
  "CLASH", "CLASS", "CLEAN", "CLEAR", "CLERK", "CLICK", "CLIFF", "CLIMB", "CLOAK", "CLOCK",
  "CLONE", "CLOSE", "CLOTH", "CLOUD", "CLOVE", "CLOWN", "COACH", "COAST", "COLOR", "COMET",
  "CORAL", "COUCH", "COUNT", "COURT", "COVER", "CRACK", "CRAFT", "CRANE", "CRASH", "CRATE",
  "CRAWL", "CRAZY", "CREAM", "CREEK", "CREST", "CRISP", "CROSS", "CROWD", "CROWN", "CRUSH",
  "CRUST", "CURVE", "CYBER", "CYCLE", "DAILY", "DANCE", "DEBUT", "DECAY", "DECOR", "DELAY",
  "DELTA", "DEMON", "DENSE", "DEPOT", "DEPTH", "DEVIL", "DIARY", "DIGIT", "DIRTY", "DISCO",
  "DIVER", "DIZZY", "DODGE", "DONOR", "DOUBT", "DRAFT", "DRAIN", "DRAKE", "DRAMA", "DREAM",
  "DRESS", "DRIFT", "DRILL", "DRINK", "DRIVE", "DRONE", "DROWN", "DRUID", "EARLY", "EARTH",
  "ECLIP", "EDICT", "ELDER", "ELBOW", "ELECT", "ELITE", "EMPTY", "ENEMY", "ENJOY", "ENTER",
  "ENTRY", "EQUAL", "EQUIP", "ERASE", "ERROR", "ERUPT", "ESSAY", "ETHER", "ETHIC", "EVENT",
  "EVERY", "EXACT", "EXALT", "EXCEL", "EXILE", "EXIST", "EXTRA", "FAINT", "FAITH", "FALSE",
  "FANCY", "FATAL", "FAULT", "FAVOR", "FEAST", "FIBER", "FIELD", "FIERCE", "FIFTH", "FIGHT",
  "FINAL", "FINCH", "FIRST", "FIXED", "FLAME", "FLASH", "FLEET", "FLESH", "FLOAT", "FLOCK",
  "FLOOD", "FLOOR", "FLOUR", "FLUTE", "FOCAL", "FOCUS", "FORGE", "FORTH", "FORTY", "FORUM",
  "FOUND", "FRAME", "FRANK", "FRAUD", "FRESH", "FRONT", "FROST", "FRUIT", "FUNNY", "GHOST",
  "GIANT", "GIVEN", "GLANCE", "GLASS", "GLEAM", "GLIDE", "GLOBE", "GLOOM", "GLORY", "GLOVE",
  "GLYPH", "GOOSE", "GRACE", "GRADE", "GRAIN", "GRAND", "GRANT", "GRAPE", "GRAPH", "GRASP",
  "GRASS", "GRAVE", "GREAT", "GREED", "GREEN", "GREET", "GRIEF", "GRILL", "GRIND", "GROVE",
  "GROWL", "GROWN", "GUARD", "GUESS", "GUEST", "GUIDE", "GUILD", "HABIT", "HARBOR", "HARSH",
  "HASTE", "HAVEN", "HEART", "HEAVY", "HEDGE", "HELLO", "HEROIC", "HIKER", "HINGE", "HOBBY",
  "HONEY", "HONOR", "HORSE", "HOTEL", "HOUND", "HOUSE", "HOVER", "HUMAN", "HUMOR", "IDEAL",
  "IMAGE", "IMPLY", "INDEX", "INERT", "INNER", "INPUT", "INTRO", "IONIC", "IRONY", "ISSUE",
  "IVORY", "JELLY", "JEWEL", "JOINT", "JUDGE", "JUICE", "JUMBO", "KNACK", "KNIFE", "KNOCK",
  "LABEL", "LABOR", "LANCE", "LARGE", "LASER", "LATCH", "LATER", "LAUGH", "LAYER", "LEAFY",
  "LEARN", "LEASE", "LEAST", "LEAVE", "LEGAL", "LEMON", "LEVEL", "LEVER", "LIGHT", "LIMIT",
  "LINEN", "LINER", "LODGE", "LOGIC", "LOVER", "LOYAL", "LUCKY", "LUNAR", "LUNCH", "MAGIC",
  "MAJOR", "MAKER", "MANGO", "MANOR", "MAPLE", "MARCH", "MATCH", "MEDAL", "MEDIA", "MERCY",
  "MERIT", "METAL", "METER", "METRO", "MICRO", "MIDST", "MIGHT", "MINER", "MINOR", "MIXER",
  "MODEL", "MODEM", "MONEY", "MONTH", "MORAL", "MOTOR", "MOUNT", "MOUSE", "MOUTH", "MOVIE",
  "MUSIC", "NAIVE", "NERVE", "NIGHT", "NOBLE", "NOISE", "NORTH", "NOTCH", "NOVEL", "NURSE",
  "OASIS", "OCEAN", "OCTET", "OFFER", "OFTEN", "OLIVE", "ONION", "ONSET", "OPERA", "OPTIC",
  "ORBIT", "ORDER", "ORGAN", "OTHER", "OTTER", "OUTER", "OZONE", "PAINT", "PANEL", "PANIC",
  "PAPER", "PARCH", "PARK", "PARTY", "PASTA", "PATCH", "PATIO", "PAUSE", "PEACE", "PEACH",
  "PEARL", "PEDAL", "PENNY", "PHASE", "PHONE", "PHOTO", "PIANO", "PILOT", "PINCH", "PIPER",
  "PIVOT", "PIXEL", "PIZZA", "PLACE", "PLAIN", "PLANE", "PLANK", "PLANT", "PLATE", "PLAZA",
  "PLEAD", "PLUCK", "PLUMB", "PLUME", "PLUSH", "POCKET", "POETIC", "POINT", "POLAR", "POLKA",
  "POPPY", "PORCH", "POUND", "POWER", "PRESS", "PRICE", "PRIDE", "PRIME", "PRINT", "PRIZE",
  "PROBE", "PROUD", "PULSE", "PUPIL", "PUPPY", "PURSE", "QUEEN", "QUERY", "QUEST", "QUICK",
  "QUIET", "QUILT", "QUIRK", "QUOTA", "RADAR", "RADIO", "RAINY", "RAISE", "RALLY", "RANCH",
  "RANGE", "RAPID", "RATIO", "RAZOR", "REACH", "REACT", "READY", "REALM", "REBEL", "RELAX",
  "RELIC", "REMEDY", "RENEW", "REPEL", "RESET", "RESIN", "RIDER", "RIDGE", "RIFLE", "RIGHT",
  "RIVAL", "RIVER", "ROBOT", "ROCKY", "RODEO", "ROGUE", "ROOFTOP", "ROPER", "ROSE", "ROUND",
  "ROUTE", "ROVER", "ROYAL", "RUBBER", "RULER", "RUMOR", "RURAL", "RUSTIC", "SADLY", "SAINT",
  "SALAD", "SALON", "SALSA", "SANDY", "SAUCE", "SCALE", "SCARE", "SCENE", "SCENT", "SCOPE",
  "SCORE", "SCOUT", "SCRAP", "SCREW", "SENOR", "SENSE", "SERVE", "SHADE", "SHADOW", "SHAFT",
  "SHAKE", "SHANK", "SHAPE", "SHARE", "SHARK", "SHARP", "SHEEP", "SHEER", "SHEET", "SHELF",
  "SHELL", "SHIFT", "SHINE", "SHIRT", "SHOCK", "SHORE", "SHORT", "SHOUT", "SIGHT", "SIGMA",
  "SILENT", "SILVER", "SIMPLE", "SIREN", "SKILL", "SKIRT", "SKULL", "SLATE", "SLEEP", "SLIDE",
  "SLOPE", "SLOTH", "SMART", "SMELL", "SMILE", "SMOKE", "SNACK", "SNAKE", "SNAP", "SOLAR",
  "SOLID", "SOLVE", "SONAR", "SOUND", "SOUTH", "SPACE", "SPARK", "SPEAK", "SPEAR", "SPEED",
  "SPELL", "SPEND", "SPICE", "SPILL", "SPINE", "SPITE", "SPLIT", "SPOKE", "SPOON", "SPORT",
  "SPRING", "SPURT", "SQUAD", "SQUARE", "STACK", "STAFF", "STAGE", "STAIN", "STAIR", "STAKE",
  "STAMP", "STAND", "STARK", "START", "STATE", "STEAM", "STEEL", "STEEP", "STEER", "STICK",
  "STILL", "STOCK", "STONE", "STOOP", "STORM", "STORY", "STRAP", "STRAW", "STRIP", "STUDY",
  "STUFF", "STUMP", "STYLE", "SUGAR", "SUITE", "SUNNY", "SUPER", "SURGE", "SWAMP", "SWARM",
  "SWEAT", "SWEEP", "SWEET", "SWIFT", "SWING", "SWORD", "TABLE", "TASTE", "TEACH", "TEMPO",
  "TIGER", "TIMBER", "TITLE", "TOAST", "TOKEN", "TONIC", "TOOTH", "TOPIC", "TORCH", "TOTAL",
  "TOUCH", "TOUGH", "TOWER", "TRACK", "TRADE", "TRAIL", "TRAIN", "TRAIT", "TRASH", "TREAT",
  "TREND", "TRIAD", "TRIAL", "TRIBE", "TRICK", "TRIP", "TROOP", "TRUCK", "TRULY", "TRUMP",
  "TRUST", "TRUTH", "TULIP", "TUMOR", "TUNER", "TURBO", "TWIST", "ULTRA", "UNCLE", "UNDER",
  "UNION", "UNIFY", "UNIT", "UNITY", "UPPER", "UPSET", "URBAN", "USAGE", "USHER", "VALID",
  "VALOR", "VALUE", "VALVE", "VAPOR", "VAULT", "VENUE", "VERB", "VERGE", "VERVE", "VIGOR",
  "VIRAL", "VIRUS", "VISIT", "VISTA", "VITAL", "VIVID", "VOCAL", "VOICE", "VORTEX", "VOUCH",
  "WAGON", "WASTE", "WATCH", "WATER", "WAVE", "WEALTH", "WEARY", "WEAVE", "WEDGE", "WEIRD",
  "WHALE", "WHEAT", "WHEEL", "WHERE", "WHILE", "WHITE", "WHOLE", "WIDOW", "WIDTH", "WINDY",
  "WITCH", "WITTY", "WORLD", "WORRY", "WORTH", "WOUND", "WRATH", "WRIST", "WRITE", "WRONG",
  "YACHT", "YARD", "YEARN", "YEAST", "YIELD", "YOUTH", "ZEBRA", "ZERO", "ZEST", "ZONAL"
].filter((w) => w.length === 5);

// Additional valid guess words (common english words)
export const ADDITIONAL_GUESSES: string[] = [
  "AAHED", "ABACK", "ABASE", "ABATE", "ABBEY", "ABBOT", "ABIDE", "ABLER", "ABODE", "ABORT",
  "ABOUT", "ABUSE", "ABYSS", "ACHED", "ACHES", "ACIDS", "ACORN", "ACRES", "ACTED", "ACTOR",
  "ACUTE", "ADAGE", "ADDED", "ADDER", "ADEPT", "ADIEU", "ADMIN", "ADORE", "ADORN", "AFFIX",
  "AFIRE", "AFOOT", "AFOUL", "AFOOT", "AFTER", "AGAPE", "AGATE", "AGAVE", "AGILE", "AGING",
  "AGLOW", "AGONY", "AGORA", "AIDES", "AIMED", "AIRER", "AISLE", "ALBUM", "ALERT", "ALGAE",
  "ALIBI", "ALIGN", "ALLAY", "ALLEY", "ALLOT", "ALLOY", "ALOFT", "ALOOF", "ALOUD", "ALPHA",
  "ALTAR", "ALTER", "AMASS", "AMAZE", "AMBER", "AMBLE", "AMEND", "AMISS", "AMITY", "AMPLE",
  "AMPLY", "AMUSE", "ANGST", "ANIME", "ANKLE", "ANNEX", "ANNOY", "ANNUL", "ANODE", "ANTIC",
  "ANVIL", "AORTA", "APING", "APNEA", "APPLE", "APPLY", "APRON", "APTLY", "ARBOR", "ARDOR",
  "AREAS", "ARGON", "ARGUE", "ARMED", "AROMA", "AROSE", "ARROW", "ARSON", "ARTSY", "ASCOT",
  "ASHEN", "ASHES", "ASKEW", "ASPEN", "ASSAY", "ATLAS", "ATOLL", "ATONE", "ATTIC", "AUNTY",
  "AURAL", "AURA", "AUTOS", "AVAIL", "AVERT", "AVIAN", "AWFUL", "AXIAL", "AXIOM", "AXLES",
  "AZURE", "BABEL", "BABES", "BACON", "BADLY", "BAGEL", "BAGGY", "BAILS", "BAIT", "BAKES",
  "BALDY", "BALED", "BALER", "BALES", "BALLS", "BALMY", "BALSA", "BANAL", "BANDS", "BANDY",
  "BANGS", "BANJO", "BANKS", "BARED", "BARER", "BARES", "BARGE", "BARKS", "BARNY", "BARON",
  "BARS", "BASAL", "BASED", "BASER", "BASES", "BASIL", "BASIN", "BASIS", "BASKS", "BATHE",
  "BATHS", "BATON", "BATTY", "BAWDY", "BAYOU", "BEADS", "BEADY", "BEAKS", "BEAMS", "BEANS",
  "BEARS", "BEATS", "BEAU", "BEAUT", "BEDEW", "BEECH", "BEEFS", "BEEFY", "BEEPS", "BEERS",
  "BEETS", "BEGET", "BEGAT", "BEGUN", "BEIGE", "BELIE", "BELLE", "BELLS", "BELLY", "BELTS",
  "BEND", "BENDS", "BERET", "BERRY", "BERTH", "BESET", "BETEL", "BEVEL", "BICEP", "BIDDY",
  "BIDES", "BIDET", "BIKES", "BILGE", "BILLS", "BILLY", "BIMBO", "BINDS", "BINGE", "BINGO",
  "BIOME", "BIRCH", "BIRDS", "BISON", "BITCH", "BITER", "BITES", "BITSY", "BITTY", "BLACK",
  "BLABS", "BLADE", "BLAHS", "BLAND", "BLANK", "BLARE", "BLASH", "BLEAK", "BLEAT", "BLEED",
  "BLEEP", "BLEND", "BLIMP", "BLING", "BLINK", "BLIPS", "BLISS", "BLITZ", "BLOAT", "BLOBS",
  "BLOKE", "BLOND", "BLOTS", "BLOWN", "BLOWS", "BLUES", "BLUFF", "BLUNT", "BLURB", "BLURS",
  "BLURT", "BLUSH", "BOARS", "BOATS", "BOBBY", "BOCCE", "BODED", "BODES", "BODYS", "BOGEY",
  "BOGGY", "BOGUS", "BOILS", "BOLAS", "BOLES", "BOLLS", "BOLTS", "BOLUS", "BOMBS", "BONDS",
  "BONED", "BONER", "BONES", "BONGO", "BONGS", "BONNY", "BONSA", "BONUS", "BOOBS", "BOOBY",
  "BOOED", "BOOKS", "BOOMS", "BOOMY", "BOONS", "BOORS", "BOOST", "BOOTS", "BOOZE", "BOOZY",
  "BORAX", "BORED", "BORER", "BORES", "BORNE", "BOSOM", "BOSON", "BOSS", "BOSSY", "BOSUN",
  "BOTCH", "BOUGH", "BOULE", "BOUND", "BOUTS", "BOWED", "BOWEL", "BOWER", "BOWLS", "BOXED",
  "BOXER", "BOXES", "BOYOS", "BRACE", "BRAGS", "BRAID", "BRAIN", "BRAKE", "BRAND", "BRASH",
  "BRATS", "BRAVO", "BRAWL", "BRAWN", "BRAYS", "BRAZE", "BREAD", "BREAK", "BREAM", "BREED",
  "BREWS", "BRIAR", "BRIBE", "BRICK", "BRIDE", "BRIEF", "BRIER", "BRIGS", "BRIMS", "BRINE",
  "BRING", "BRINK", "BRINY", "BRISK", "BROAD", "BROIL", "BROKE", "BROOD", "BROOK", "BROOM",
  "BROTH", "BROWN", "BROWS", "BRUIN", "BRUNT", "BRUSH", "BRUTE", "BUBBA", "BUCKS", "BUDDY",
  "BUDGE", "BUFFS", "BUGGY", "BUGLE", "BUILDS", "BUILT", "BULBS", "BULGE", "BULGY", "BULKS",
  "BULKY", "BULLS", "BULLY", "BUMPS", "BUMPY", "BUNCH", "BUNKS", "BUNNY", "BUOYS", "BURGS",
  "BURLY", "BURNS", "BURNT", "BURPS", "BURRO", "BURRS", "BURSA", "BURST", "BUSES", "BUSHY",
  "BUSTS", "BUSTY", "BUTCH", "BUTTE", "BUTTS", "BUXOM", "BUYER", "BUZZY", "BYLAW", "BYWAY",
  "CABAL", "CABBY", "CABER", "CABIN", "CABLE", "CACAO", "CACHE", "CACTI", "CADDY", "CADET",
  "CADRE", "CAFES", "CAGED", "CAGES", "CAGEY", "CAIRN", "CAKES", "CAKEY", "CALLS", "CALMS",
  "CALVE", "CAMEL", "CAMEO", "CAMPS", "CAMPY", "CANAL", "CANDY", "CANES", "CANID", "CANNA",
  "CANNY", "CANOE", "CANON", "CANTO", "CAPED", "CAPER", "CAPES", "CAPON", "CAPOS", "CAPUT",
  "CARAT", "CARBO", "CARBS", "CARDS", "CARED", "CARER", "CARES", "CARGO", "CAROB", "CAROL",
  "CAROM", "CARPS", "CARRY", "CARTE", "CARTS", "CARVE", "CASAS", "CASED", "CASES", "CASKS",
  "CASTE", "CASTS", "CATCH", "CATER", "CATTY", "CAULK", "CAUSE", "CAVED", "CAVER", "CAVES",
  "CAVIL", "CEASE", "CEDAR", "CEDED", "CEDER", "CEDES", "CELEB", "CELLO", "CELLS", "CELTS",
  "CENTS", "CHAFE", "CHAFF", "CHAIN", "CHAIR", "CHALK", "CHAMP", "CHANT", "CHAOS", "CHAPS",
  "CHARD", "CHARM", "CHARS", "CHART", "CHASE", "CHASM", "CHATS", "CHEAP", "CHEAT", "CHECK",
  "CHEEK", "CHEEP", "CHEER", "CHEFS", "CHEMO", "CHESS", "CHEST", "CHEWS", "CHEWY", "CHICK",
  "CHIDE", "CHIEF", "CHILD", "CHILI", "CHILL", "CHIME", "CHIMP", "CHINA", "CHINE", "CHINK",
  "CHINO", "CHINS", "CHIPS", "CHIRP", "CHIVE", "CHOCK", "CHOIR", "CHOKE", "CHOMP", "CHORD",
  "CHORE", "CHOSE", "CHOWS", "CHUCK", "CHUMP", "CHUNK", "CHURN", "CHUTE", "CIDER", "CIGAR",
  "CILIA", "CINCH", "CIRCA", "CITES", "CIVIC", "CIVIL", "CLACK", "CLAIM", "CLAMP", "CLAMS",
  "CLANG", "CLANK", "CLANS", "CLAPS", "CLASH", "CLASP", "CLASS", "CLAVE", "CLAWS", "CLAYS",
  "CLEAN", "CLEAR", "CLEAT", "CLEFS", "CLEFT", "CLERK", "CLEWS", "CLICK", "CLIFF", "CLIMB",
  "CLIME", "CLING", "CLINK", "CLIPS", "CLOAK", "CLOCK", "CLODS", "CLOGS", "CLONE", "CLOOT",
  "CLOSE", "CLOTH", "CLOTS", "CLOUD", "CLOUT", "CLOVE", "CLOWN", "CLUBS", "CLUCK", "CLUED",
  "CLUES", "CLUMP", "CLUNG", "COACH", "COALS", "COAST", "COATS", "COBIA", "COBRA", "COCKS",
  "COCKY", "COCOA", "CODAS", "CODED", "CODER", "CODES", "CODEX", "CODON", "COEDS", "COHOE",
  "COIFS", "COILS", "COINS", "COLAS", "COLDS", "COLES", "COLIC", "COLIN", "COLON", "COLOR",
  "COLTS", "COMAS", "COMBO", "COMBS", "COMER", "COMES", "COMET", "COMFY", "COMIC", "COMMA",
  "CONCH", "CONDO", "CONES", "CONGA", "CONGO", "CONIC", "CONKS", "COOED", "COOKS", "COOKY",
  "COOLS", "COOPS", "COOPT", "COPES", "COPRA", "COPSE", "CORAL", "CORDS", "CORED", "CORER",
  "CORES", "CORGI", "CORKS", "CORKY", "CORNS", "CORNY", "CORPS", "COSTS", "COTES", "COUCH",
  "COUGH", "COULD", "COUNT", "COUPE", "COUPS", "COURT", "COVEN", "COVER", "COVES", "COVET",
  "COVEY", "COWED", "COWER", "COWLS", "CRABS", "CRACK", "CRAFT", "CRAGS", "CRAMP", "CRAMS",
  "CRANE", "CRANK", "CRAPS", "CRASH", "CRASS", "CRATE", "CRAVE", "CRAWL", "CRAWS", "CRAZE",
  "CRAZY", "CREAK", "CREAM", "CREDO", "CREED", "CREEK", "CREEP", "CREPE", "CREPT", "CRESS",
  "CREST", "CREWS", "CRIBS", "CRICK", "CRIED", "CRIER", "CRIES", "CRIME", "CRIMP", "CRISP",
  "CRITS", "CROAK", "CROCK", "CROFT", "CRONE", "CRONY", "CROOK", "CROON", "CROPS", "CROSS",
  "CROUP", "CROWD", "CROWN", "CROWS", "CRUDE", "CRUEL", "CRUET", "CRUMB", "CRUMP", "CRUSH",
  "CRUST", "CRYPT", "CUBBY", "CUBED", "CUBES", "CUBIC", "CUBIT", "CUFFS", "CUING", "CULLS",
  "CULPA", "CULT", "CUMIN", "CUPID", "CUPPA", "CURBS", "CURDS", "CURED", "CURER", "CURES",
  "CURIA", "CURIO", "CURLS", "CURLY", "CURRY", "CURSE", "CURVE", "CURVY", "CUSHY", "CUSPS",
  "CUTER", "CUTES", "CUTIE", "CUTIS", "CYBER", "CYCAD", "CYCLE", "CYCLO", "CYNIC", "CYSTS"
].filter((w) => w.length === 5);

export const ALL_VALID_WORDS: Set<string> = new Set([
  ...TARGET_WORDS,
  ...ADDITIONAL_GUESSES,
]);

export function isValidWord(word: string): boolean {
  if (!word || word.length !== 5) return false;
  return ALL_VALID_WORDS.has(word.toUpperCase());
}

export function evaluateGuess(guess: string, target: string): TileState[] {
  const g = guess.toUpperCase().split("");
  const t = target.toUpperCase().split("");
  const result: TileState[] = Array(5).fill("absent");
  const targetCounts: Record<string, number> = {};

  // First pass: identify exact position matches
  for (let i = 0; i < 5; i++) {
    if (g[i] === t[i]) {
      result[i] = "correct";
    } else {
      targetCounts[t[i]] = (targetCounts[t[i]] || 0) + 1;
    }
  }

  // Second pass: identify letters present elsewhere
  for (let i = 0; i < 5; i++) {
    if (result[i] === "correct") continue;
    const letter = g[i];
    if (targetCounts[letter] && targetCounts[letter] > 0) {
      result[i] = "present";
      targetCounts[letter]--;
    }
  }

  return result;
}

export function getDailyWord(dateString?: string): { word: string; dayIndex: number; dateStr: string } {
  const today = dateString ? new Date(dateString) : new Date();
  const dateStr = today.toISOString().slice(0, 10);
  
  // Deterministic daily index from epoch 2024-01-01
  const epoch = new Date("2024-01-01T00:00:00Z").getTime();
  const current = new Date(`${dateStr}T00:00:00Z`).getTime();
  const dayIndex = Math.max(0, Math.floor((current - epoch) / (1000 * 60 * 60 * 24)));
  
  const wordIndex = dayIndex % TARGET_WORDS.length;
  return {
    word: TARGET_WORDS[wordIndex]!,
    dayIndex,
    dateStr,
  };
}

export function getRandomWord(): string {
  const index = Math.floor(Math.random() * TARGET_WORDS.length);
  return TARGET_WORDS[index]!;
}
