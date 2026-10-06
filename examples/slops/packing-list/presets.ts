export const DEFAULT_CATEGORIES = [
  { key: "docs", name: "Documents & Valuables", tagCode: "DOCS", color: "cobalt" },
  { key: "carry", name: "Carry-on Essentials", tagCode: "CARRY", color: "vermilion" },
  { key: "tech", name: "Tech & Electronics", tagCode: "TECH", color: "emerald" },
  { key: "toiletries", name: "Toiletries & Meds", tagCode: "TOILET", color: "amber" },
  { key: "clothing", name: "Clothing & Footwear", tagCode: "WEAR", color: "plum" },
  { key: "gear", name: "Gear & Misc", tagCode: "GEAR", color: "slate" },
];

export type Preset = {
  id: string;
  name: string;
  description: string;
  items: Array<{ key: string; text: string; quantity: number; essential?: boolean }>;
};

export const PRESETS: Preset[] = [
  {
    id: "weekend",
    name: "Weekend Getaway",
    description: "Light 3-day essentials for a quick city break or road trip.",
    items: [
      { key: "docs", text: "ID / Driver's License", quantity: 1, essential: true },
      { key: "docs", text: "Credit Cards & Small Cash", quantity: 1, essential: true },
      { key: "carry", text: "Light Daypack / Tote", quantity: 1 },
      { key: "carry", text: "Sunglasses & Case", quantity: 1 },
      { key: "tech", text: "Phone Charger & Cable", quantity: 1, essential: true },
      { key: "tech", text: "Power Bank", quantity: 1 },
      { key: "toiletries", text: "Toothbrush & Paste", quantity: 1, essential: true },
      { key: "toiletries", text: "Daily Medication", quantity: 1, essential: true },
      { key: "toiletries", text: "Deodorant", quantity: 1 },
      { key: "clothing", text: "T-Shirts / Tops", quantity: 3 },
      { key: "clothing", text: "Pants / Shorts", quantity: 2 },
      { key: "clothing", text: "Underwear & Socks", quantity: 3, essential: true },
      { key: "clothing", text: "Light Jacket / Layer", quantity: 1 },
    ],
  },
  {
    id: "international",
    name: "International Flight",
    description: "Long-haul flight checklist with travel documents, electronics, and customs prep.",
    items: [
      { key: "docs", text: "Passport (valid 6+ months)", quantity: 1, essential: true },
      { key: "docs", text: "Boarding Passes & Visas", quantity: 1, essential: true },
      { key: "docs", text: "Travel Insurance Card", quantity: 1, essential: true },
      { key: "docs", text: "Emergency Contact / Embassy Info", quantity: 1 },
      { key: "carry", text: "Noise-Canceling Headphones", quantity: 1 },
      { key: "carry", text: "Neck Pillow & Eye Mask", quantity: 1 },
      { key: "carry", text: "Empty Water Bottle", quantity: 1 },
      { key: "carry", text: "Pen for Customs Forms", quantity: 1 },
      { key: "tech", text: "Universal Plug Adapter", quantity: 2, essential: true },
      { key: "tech", text: "Laptop & Charging Brick", quantity: 1 },
      { key: "tech", text: "E-Reader / Tablet", quantity: 1 },
      { key: "toiletries", text: "TSA Liquid Toiletry Bag", quantity: 1, essential: true },
      { key: "toiletries", text: "Hand Sanitizer & Lip Balm", quantity: 1 },
      { key: "toiletries", text: "Prescriptions & Pain Relievers", quantity: 1, essential: true },
      { key: "clothing", text: "Spare Outfit in Carry-on", quantity: 1, essential: true },
    ],
  },
  {
    id: "outdoor",
    name: "Outdoor & Hiking",
    description: "Trail-ready gear, sun protection, hydration, and weather layers.",
    items: [
      { key: "gear", text: "Hiking Daypack (20-30L)", quantity: 1, essential: true },
      { key: "gear", text: "Trekking Poles", quantity: 2 },
      { key: "gear", text: "Headlamp & Extra Batteries", quantity: 1, essential: true },
      { key: "gear", text: "Multi-tool / Pocket Knife", quantity: 1 },
      { key: "toiletries", text: "First Aid Kit & Blister Pads", quantity: 1, essential: true },
      { key: "toiletries", text: "Sunscreen SPF 50 & Bug Spray", quantity: 1, essential: true },
      { key: "carry", text: "Hydration Bladder / Bottle", quantity: 2, essential: true },
      { key: "carry", text: "Trail Snacks & Electrolytes", quantity: 4 },
      { key: "clothing", text: "Breathable Trail Shoes", quantity: 1, essential: true },
      { key: "clothing", text: "Waterproof Rain Shell", quantity: 1, essential: true },
      { key: "clothing", text: "Merino Wool Socks", quantity: 3 },
      { key: "clothing", text: "UV Hat / Cap", quantity: 1 },
    ],
  },
  {
    id: "business",
    name: "Business Travel",
    description: "Professional wardrobe, presentation gear, and mobile office supplies.",
    items: [
      { key: "docs", text: "Work ID / Conference Badge", quantity: 1, essential: true },
      { key: "docs", text: "Business Cards Case", quantity: 1 },
      { key: "tech", text: "Work Laptop & USB-C Dock", quantity: 1, essential: true },
      { key: "tech", text: "Presentation Clicker & HDMI Cable", quantity: 1 },
      { key: "tech", text: "Phone & Watch Chargers", quantity: 1, essential: true },
      { key: "clothing", text: "Blazer / Suit Jacket", quantity: 1, essential: true },
      { key: "clothing", text: "Pressed Dress Shirts", quantity: 3 },
      { key: "clothing", text: "Dress Shoes & Shoe Bags", quantity: 1 },
      { key: "clothing", text: "Belt & Watch", quantity: 1 },
      { key: "carry", text: "Travel Garment Steamer", quantity: 1 },
      { key: "toiletries", text: "Hair Styling & Cologne", quantity: 1 },
    ],
  },
];
