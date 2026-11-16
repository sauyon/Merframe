import { useState } from "react";

export interface Quote {
  line: string;
  speaker: string;
}

export const QUOTES = {
  startup: [
    {
      line: "Oi fancy, we gonna do this or what?",
      speaker: "Boon",
    },
    {
      line: "Come to bask in my presence, or do you have a purpose for interrupting me?",
      speaker: "Roathe",
    },
    {
      line: "Go and cause trouble.",
      speaker: "Daughter",
    },
    {
      line: "I'm... I'm getting control. Hold on. I can help too.",
      speaker: "Silvana",
    },
    {
      line: "I was wondering if you would come by today.",
      speaker: "Marie",
    },
  ],
  world: [
    {
      line: "Ordis has been counting stars, Operator. All accounted for.",
      speaker: "Ordis",
    },
    {
      line: "Grineer galleons kickin' up dust all the way from Venus to Pluto. Watch your backs, people.",
      speaker: "Nora Night",
    },
    {
      line: "You exist on the fold between two worlds. The world we know, of blood and steel, and the world that watches and dreams, the Void.",
      speaker: "Teshin",
    },
    {
      line: "Got a few seconds before the system resets. We can take them again. You game?",
      speaker: "Little Duck",
    },
    {
      line: "We never see the stars here, but tonight we'll remember them.",
      speaker: "Grandmother",
    },
  ],
  resetChecklist: [
    {
      line: "And whose business are you about today, Drifter?",
      speaker: "Acrithis",
    },
    {
      line: "Busy day in the garden. Lots of jobs. You finish one, come back, I got more.",
      speaker: "Nightcap",
    },
    {
      line: "Another day, another relay.",
      speaker: "Aspirant Zorba",
    },
    {
      line: "Sorry, Pal. No new leads today. But listen, I've got my ear to the ground on Ayatans, don't worry.",
      speaker: "Maroo",
    },
    {
      line: "Ah well. Same time tomorrow?",
      speaker: "Ticker",
    },
  ],
  arbitrations: [
    {
      line: "Fight this Arbitration as any mortal creature would. There are no second chances.",
      speaker: "Arbiters of Hexis",
    },
    {
      line: "Only under the shadow of death will your true potential shine.",
      speaker: "Arbiters of Hexis",
    },
    {
      line: "For this Arbitration, we bestow upon you the gift of mortality.",
      speaker: "Arbiters of Hexis",
    },
    {
      line: "May you live until you die.",
      speaker: "Arbiters of Hexis",
    },
    {
      line: "Have you survived Arbitration? Honors await you.",
      speaker: "Arbiters of Hexis",
    },
  ],
  foundry: [
    {
      line: "Operator? Ordis has been interfacing with the Foundry's AI Precepts. You could say we forged a new connection.",
      speaker: "Ordis",
    },
    {
      line: "Now this is the kind of thing you can produce if you're willing to put in the hours. I'm telling ya, most don't have the patience.",
      speaker: "Father",
    },
    {
      line: "There is no rush; Hok has... all day.",
      speaker: "Hok",
    },
    {
      line: "Take your time. I live to serve.",
      speaker: "Rude Zuud",
    },
    {
      line: "Here's a few pieces I put together in my own time. Void knows I got plenty of that.",
      speaker: "Cavalero",
    },
  ],
  inventory: [
    {
      line: "Do you need a bag for all of these? Just kidding, I'll have them sent to your armory.",
      speaker: "Darvo",
    },
    {
      line: "Inventory, secrets and scars. No need to put them all on display.",
      speaker: "Varzia",
    },
    {
      line: "Every trinket a story! A story waiting to be shared, my lovely. A story waiting to be freed.",
      speaker: "Ticker",
    },
    {
      line: "Until next time, traveler! May your HP stay full and your inventory stay encumbered.",
      speaker: "Amir",
    },
    {
      line: "Is your mod collection in order? No, I am not equipped to feel envy...",
      speaker: "Ordis",
    },
  ],
  relicPlanner: [
    {
      line: "All my wares come from the Void. Perhaps you'll get to visit sometime.",
      speaker: "Baro Ki'Teer",
    },
    {
      line: "They say fortune favours the bold. Feeling lucky today, Tenno?",
      speaker: "Baro Ki'Teer",
    },
    {
      line: "Can't ever be sure what I'll trawl up. Seems to come in waves.",
      speaker: "Varzia",
    },
    {
      line: "Let's see what twisted wonders the Void has created.",
      speaker: "Archimedean Yonta",
    },
    {
      line: "Tenno use the keys, but they are mere trespassers. Only I, Vor, know the true power of the Void.",
      speaker: "Captain Vor",
    },
  ],
  rivens: [
    {
      line: "New strength creates new weakness. Pupil, can you balance these opposing forces?",
      speaker: "Teshin",
    },
    {
      line: "You seek power, you pay in blood.",
      speaker: "Teshin",
    },
    {
      line: "A pinch of the Void, and any weapon can be made to sing a new tune.",
      speaker: "Cavalero",
    },
    {
      line: "The Amp is an extension of your will. No part of it may be chosen lightly.",
      speaker: "Onkko",
    },
    {
      line: "I wouldn't have become the most successful Void Trader in the Origin System without relishing the call of uncertainty.",
      speaker: "Baro Ki'Teer",
    },
  ],
  mastery: [
    {
      line: "My pupil, the next summit awaits. Are you ready to make the climb?",
      speaker: "Teshin",
    },
    {
      line: "Congratulations, True Master.",
      speaker: "Lotus",
    },
    {
      line: "Might I interest you in a gun that requires no skill to operate but a lifetime to master?",
      speaker: "Amir",
    },
    {
      line: "That little plinker ain't fired near enough. Come back when you know its worth, then we can talk gildin'.",
      speaker: "Rude Zuud",
    },
    {
      line: "The path to greatness is walked one step at a time.",
      speaker: "Nora Night",
    },
  ],
  resources: [
    {
      line: "Scans indicate these conduits are connected to a massive stockpile of resources. Just how long is the Operator planning to fight for?",
      speaker: "Ordis",
    },
    {
      line: "Mining isn't my only interest. I'm also interested in mining.",
      speaker: "Otak",
    },
    {
      line: "This decision is ill-advised. The pursuit of mineralogy can make a person very happy indeed.",
      speaker: "Smokefinger",
    },
    {
      line: "Excavation complete. A satisfactory haul.",
      speaker: "Loid",
    },
    {
      line: "Take your pick of our cargo! It's precious little use to us.",
      speaker: "Grandmother",
    },
  ],
  market: [
    {
      line: "Welcome to my little shop.",
      speaker: "Darvo",
    },
    {
      line: "Do have a look, who knows, you may turn into a paying customer someday.",
      speaker: "Baro Ki'Teer",
    },
    {
      line: "Get over here doll. Pre-loved and second-hand. Treasures that deserve a second chance.",
      speaker: "Ticker",
    },
    {
      line: "Others have one price for locals and one for offworlders. Me? I give you the local price, every time!",
      speaker: "Nakak",
    },
    {
      line: "These are a bargain, just for you.",
      speaker: "Ergo Glast",
    },
  ],
  marketListings: [
    {
      line: "Browsing is always free. How fortunate for you.",
      speaker: "Baro Ki'Teer",
    },
    {
      line: "Ah, so you're buying? That's nice. People look out for Tenno with something to sell. And remember, only suckers pay the first price.",
      speaker: "Maroo",
    },
    {
      line: "Woah, you already got a deal. No hoarding.",
      speaker: "Darvo",
    },
    {
      line: "The once-loved in need of a little TLC, Stardust. I hope you came here with credits and good intentions.",
      speaker: "Ticker",
    },
    {
      line: "You have yourself a deal.",
      speaker: "Mother",
    },
  ],
  analytics: [
    {
      line: "I have this cycle's figures, and they are very pleasing!",
      speaker: "Nef Anyo",
    },
    {
      line: "I only work for myself. I make my own profits.",
      speaker: "Darvo",
    },
    {
      line: "Do you ever wonder what I do with all your ducats? Well, keep wondering.",
      speaker: "Baro Ki'Teer",
    },
    {
      line: "When you don't run with any of the major factions or Syndicates, the big pay-days are few and far between.",
      speaker: "Maroo",
    },
    {
      line: "Life's just a bargain like any other.",
      speaker: "Parvos Granum",
    },
  ],
  overlays: [
    {
      line: "It is always satisfying to watch you work, Operator.",
      speaker: "Ordis",
    },
    {
      line: "We watch, we anticipate, we intercede.",
      speaker: "Onkko",
    },
    {
      line: "Tenno, I am detecting a Synthesis target. Use the Synthesis Scanner to track the creature.",
      speaker: "Cephalon Simaris",
    },
    {
      line: "Well done. Eyes and ears remain online, and our next move is clear.",
      speaker: "Loid",
    },
    {
      line: "Eyes open. We do not know what is on that thing.",
      speaker: "Cephalon Cy",
    },
  ],
  stats: [
    {
      line: "All beings of substance die eventually. But only those forgotten are truly dead.",
      speaker: "Cephalon Simaris",
    },
    {
      line: "Advancement imminent. I require more data.",
      speaker: "Cephalon Suda",
    },
    {
      line: "The kicker isn't that you kids brought down an Empire. It's that half of you don't remember doing it.",
      speaker: "Varzia",
    },
    {
      line: "Reconstruction of the past is possible.",
      speaker: "Archimedean Yonta",
    },
    {
      line: "You don't have to keep what you catch. I'm happy to find a home for it in my archive.",
      speaker: "Daughter",
    },
  ],
  settings: [
    {
      line: "Sorry! Ordis will play with the volume off.",
      speaker: "Ordis",
    },
    {
      line: "Operator, should I prepare the Orbiter for disinfection?",
      speaker: "Ordis",
    },
    {
      line: "Hope you don't mind - I rearranged some cables! Tidy space, tidy mind.",
      speaker: "Aoi",
    },
    {
      line: "Such a mess they've made. And I suppose it's down to you and I to tidy up. Shall we?",
      speaker: "Grandmother",
    },
    {
      line: "Adjust your weaponry if you see fit.",
      speaker: "Hunhow",
    },
  ],
  about: [
    {
      line: "You are the Tenno. You are the Operator. Ordis is the Cephalon. Ordis is the ship.",
      speaker: "Ordis",
    },
    {
      line: "Pupil, if your Warframe becomes a useless husk without you, then what do you become without it?",
      speaker: "Teshin",
    },
    {
      line: "I am named, and my light is my own. It is not a debt I owe to the dust.",
      speaker: "Mother",
    },
    {
      line: "You are an instrument most perfect, Tenno. Tuned to the pitch of the Unum.",
      speaker: "Onkko",
    },
    {
      line: "Style baby. It is all about knowing who you are and not giving a damn.",
      speaker: "Nora Night",
    },
  ],
} satisfies Record<string, readonly Quote[]>;

export type PageQuoteKey = keyof typeof QUOTES;

export function usePageQuote(key: PageQuoteKey): Quote {
  const [quote] = useState(() => {
    const options = QUOTES[key];
    return options[Math.floor(Math.random() * options.length)] as Quote;
  });
  return quote;
}
