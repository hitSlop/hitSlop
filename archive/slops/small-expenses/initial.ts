import type { Input } from "@hitslop/document";
import { expenses } from "./schema";
export default {
  title:"The little things",
  currency:"CAD",
  items:[
    {merchant:"Neighbourhood coffee",amountMinor:475,note:"Morning catch-up",settled:false},
    {merchant:"Studio supplies",amountMinor:2400,settled:false},
    {merchant:"Train home",amountMinor:650,settled:true},
  ],
} satisfies Input<typeof expenses.fields.node>;
