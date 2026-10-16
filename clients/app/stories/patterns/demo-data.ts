// Fictional demo data from the approved prototypes (issue #23). Every name,
// company, address and amount here is invented.
import type { IconName, Signal } from "~/lib/ui";

export interface QueueItemData {
  id: string;
  signal: Signal;
  who: string;
  name: string;
  service?: boolean;
  due: string;
  soon?: boolean;
  what: string;
  tags: string[];
  quoteFrom: string;
  quote: string;
  context: string;
  buttons: {
    label: string;
    variant: "primary" | "secondary" | "danger";
    keys?: string;
    shortcut?: string;
  }[];
}

export interface WorkspaceData {
  key: "business" | "personal";
  name: string;
  mark: string;
  kind: string;
  unread: number;
  tasks: number;
  sorted: number;
  date: string;
  streams: { name: string; count: number; fresh: string }[];
  today: { time: string; title: string; where: string; signal: Signal }[];
  money: {
    due: string;
    dueLabel: string;
    incoming: string;
    incomingLabel: string;
    statement: string;
  };
  waiting: { name: string; service?: boolean; text: string }[];
  log: string[];
  items: QueueItemData[];
}

export const SIGNAL_ORDER: { signal: Signal; name: string }[] = [
  { signal: "review", name: "Review" },
  { signal: "reply", name: "Reply" },
  { signal: "decide", name: "Decide" },
  { signal: "do", name: "Do" },
  { signal: "pay", name: "Pay" },
  { signal: "sort", name: "Sort" },
];

export const WORKSPACES: Record<"business" | "personal", WorkspaceData> = {
  business: {
    key: "business",
    name: "Duarte & Pessoa",
    mark: "DP",
    kind: "Business",
    unread: 14,
    tasks: 7,
    sorted: 48,
    date: "Monday 12 October",
    streams: [
      { name: "Primary", count: 12, fresh: "3 new" },
      { name: "Updates", count: 31, fresh: "12 new" },
      { name: "Receipts", count: 8, fresh: "4 filed" },
      { name: "Newsletters", count: 46, fresh: "18 new" },
      { name: "Clients", count: 4, fresh: "Your rule" },
    ],
    today: [
      { time: "09:00", title: "Held: Q4 review", where: "Focus", signal: "do" },
      { time: "10:00", title: "Call with Brightloop", where: "Video", signal: "reply" },
      { time: "14:00", title: "Parcel delivery window", where: "Office", signal: "decide" },
      { time: "17:30", title: "Rui Matos, Q3 VAT", where: "Their office", signal: "pay" },
    ],
    money: {
      due: "€460.40",
      dueLabel: "2 bills due this week",
      incoming: "€2,340.00",
      incomingLabel: "Brightloop INV-0042, due 20 Oct",
      statement: "Statement of 8 Oct: 31 matched, 3 to check",
    },
    waiting: [
      { name: "Inês Carvalho", text: "Inês Carvalho: logo SVGs, 8 days" },
      { name: "Brightloop", service: true, text: "Brightloop: purchase order number, 3 days" },
    ],
    log: [
      "Filed 4 receipts in Documents",
      "Moved 2 newsletters out of Primary",
      "Made a task: review the Q4 proposal by Thursday",
      "Matched 31 bank lines to invoices",
    ],
    items: [
      {
        id: "b5",
        signal: "review",
        who: "Bankly",
        name: "Bankly",
        service: true,
        due: "Today",
        soon: true,
        what: "New sign-in from Lisbon, Sunday 21:14, on a Windows laptop",
        tags: ["Security alert", "Not a device Campbooks has seen"],
        quoteFrom: "Bankly, Sunday 21:15",
        quote:
          "We noticed a new sign-in to your business account from Lisbon on a Windows laptop. If this was you, there is nothing to do.",
        context:
          "If it was not you, Campbooks opens Bankly security and keeps this here until it is resolved.",
        buttons: [
          { label: "It was me", variant: "secondary", keys: "1" },
          { label: "It was not me", variant: "danger", keys: "2" },
        ],
      },
      {
        id: "b1",
        signal: "reply",
        who: "Sofia Pessoa · Brightloop",
        name: "Sofia Pessoa",
        due: "Thu",
        what: "Waiting on your review of the Q4 proposal; offers a call on Thursday",
        tags: ["2 messages", "Brightloop-Q4-scope-v3.pdf · 2.4 MB", "€18,500 proposal"],
        quoteFrom: "Sofia, Friday 17:42",
        quote:
          "Attached the revised scope. We moved the launch to 30 November and kept the price at €18,500. Thursday works for a call if that is easier; otherwise comments in the doc are great.",
        context:
          "Version 3 moves the timeline two weeks; the price is unchanged. You held Thursday 09:00 for it.",
        buttons: [
          { label: "Open proposal", variant: "primary", shortcut: "list.open" },
          { label: "Reply", variant: "secondary", shortcut: "item.reply" },
        ],
      },
      {
        id: "b7",
        signal: "reply",
        who: "Joana Reis · Atelier Norte",
        name: "Joana Reis",
        due: "Fri",
        what: "Needs the signed NDA back before the kickoff on Friday",
        tags: ["NDA-AtelierNorte-2026.pdf", "Needs your signature"],
        quoteFrom: "Joana, today 08:12",
        quote:
          "Could you send the NDA back signed before Friday? We cannot share the brand files until then.",
        context: "The NDA is unsigned in Documents. Campbooks can prepare it for signature.",
        buttons: [
          { label: "Sign and send", variant: "primary", shortcut: "list.open" },
          { label: "Reply", variant: "secondary", shortcut: "item.reply" },
        ],
      },
      {
        id: "b2",
        signal: "decide",
        who: "Marta Duarte",
        name: "Marta Duarte",
        due: "Wed",
        what: "Pick a week for the office move: 2 to 6 Nov or 16 to 20 Nov",
        tags: ["Landlord quote · €1,150", "Clashes with your Porto trip"],
        quoteFrom: "Marta, Saturday 09:30",
        quote:
          "Two options from the landlord: 2 to 6 November or 16 to 20 November. The movers quoted €1,150 either way. Which one works for you?",
        context: "You are in Porto from 16 to 18 November.",
        buttons: [
          { label: "2 to 6 Nov", variant: "primary", keys: "1" },
          { label: "16 to 20 Nov, clashes with Porto", variant: "secondary", keys: "2" },
        ],
      },
      {
        id: "b8",
        signal: "decide",
        who: "Tiago Mendes · Brightloop",
        name: "Tiago Mendes",
        due: "Today",
        soon: true,
        what: "Asks to move Thursday’s call to Friday 11:00",
        tags: ["Friday 11:00 is free", "Invite update"],
        quoteFrom: "Tiago, today 09:05",
        quote: "Something came up on Thursday. Could we do Friday at 11:00 instead? Same link.",
        context: "Your Friday 11:00 is free; Thursday 09:00 stays held for the review.",
        buttons: [
          { label: "Accept Friday 11:00", variant: "primary", keys: "1" },
          { label: "Suggest another time", variant: "secondary", keys: "2" },
        ],
      },
      {
        id: "b3",
        signal: "do",
        who: "Rui Matos · Contabilidade RM",
        name: "Rui Matos",
        due: "Fri",
        what: "Send two August receipts for the Q3 VAT return",
        tags: ["2 receipts in Documents", "€12.90 + €47.20"],
        quoteFrom: "Rui, Monday 11:02",
        quote:
          "Still need the two August receipts from Papelaria Sol. Friday at the latest, please, so I can file on Monday.",
        context: "Both receipts are already in Documents, matched to your August statement.",
        buttons: [
          { label: "Send both receipts", variant: "primary", shortcut: "list.open" },
          { label: "Hold time", variant: "secondary", shortcut: "item.hold" },
        ],
      },
      {
        id: "b4",
        signal: "pay",
        who: "Cloudhost",
        name: "Cloudhost",
        service: true,
        due: "Fri · €148.00",
        what: "Invoice CH-20418 for October hosting",
        tags: ["CH-20418.pdf", "Card or transfer"],
        quoteFrom: "Cloudhost, 12 October",
        quote:
          "Your invoice CH-20418 for €148.00 is due on 16 October. Pay by card or bank transfer.",
        context:
          "Not on your last statement (8 Oct). Campbooks will match it when the next one arrives.",
        buttons: [
          { label: "Mark as paid", variant: "primary", shortcut: "item.choice1" },
          { label: "Open invoice", variant: "secondary", shortcut: "list.open" },
        ],
      },
      {
        id: "b9",
        signal: "pay",
        who: "Seguros Atlântico",
        name: "Seguros Atlântico",
        service: true,
        due: "31 Oct · €312.40",
        what: "Office insurance renews; premium up 6% from last year",
        tags: ["Policy 88-20417", "+€17.70 vs 2025"],
        quoteFrom: "Seguros Atlântico, 9 October",
        quote: "Your office policy renews on 31 October. The annual premium is €312.40.",
        context: "Last year you paid €294.70. Campbooks found no claims on this policy.",
        buttons: [
          { label: "Pay now", variant: "primary", shortcut: "item.choice1" },
          { label: "Compare quotes", variant: "secondary", shortcut: "list.open" },
        ],
      },
      {
        id: "b6",
        signal: "sort",
        who: "Ana Lopes · Oficina Criativa",
        name: "Ana Lopes",
        due: "Mon",
        what: "Not sure where this goes: Primary or Updates?",
        tags: ["First message from this sender", "61% sure"],
        quoteFrom: "Ana Lopes, Monday 08:20",
        quote:
          "Workshop on 20 October: a few seats left for the morning session. Reply if you want one.",
        context: "Your choice also teaches Campbooks for this sender.",
        buttons: [
          { label: "Primary", variant: "secondary", keys: "1" },
          { label: "Updates", variant: "secondary", keys: "2" },
        ],
      },
    ],
  },
  personal: {
    key: "personal",
    name: "Ana Duarte",
    mark: "AD",
    kind: "Personal",
    unread: 5,
    tasks: 3,
    sorted: 21,
    date: "Monday 12 October",
    streams: [
      { name: "Primary", count: 6, fresh: "2 new" },
      { name: "Updates", count: 14, fresh: "5 new" },
      { name: "Receipts", count: 5, fresh: "2 filed" },
      { name: "Newsletters", count: 22, fresh: "9 new" },
    ],
    today: [
      { time: "08:15", title: "School run", where: "Kids", signal: "do" },
      { time: "19:00", title: "Gym, upper body", where: "Ginásio Alto", signal: "do" },
      { time: "20:30", title: "Dinner at Teresa’s", where: "Her place", signal: "reply" },
    ],
    money: {
      due: "€62.18",
      dueLabel: "Electricity, direct debit 20 Oct",
      incoming: "€1,240.00",
      incomingLabel: "Tax refund, expected November",
      statement: "Statement of 5 Oct: all 42 lines explained",
    },
    waiting: [
      {
        name: "Oficina do Bairro",
        service: true,
        text: "Oficina do Bairro: car repair quote, 4 days",
      },
    ],
    log: [
      "Filed the electricity bill in Documents",
      "Unsubscribed from 3 shops you never open",
      "Added parents’ evening on 22 Oct to Schedule",
    ],
    items: [
      {
        id: "p1",
        signal: "reply",
        who: "Teresa Duarte",
        name: "Teresa Duarte",
        due: "Sat",
        what: "Lunch on Sunday at one; bring the kids if they are free",
        tags: ["Family", "Your Sunday is free"],
        quoteFrom: "Teresa, Saturday 18:05",
        quote:
          "Lunch on Sunday at one? Bring the kids if they are free. I am making the cod you like.",
        context: "Your Sunday is free.",
        buttons: [
          { label: "Reply yes", variant: "primary", shortcut: "item.reply" },
          { label: "Hold Sunday", variant: "secondary", shortcut: "item.hold" },
        ],
      },
      {
        id: "p4",
        signal: "reply",
        who: "Escola Básica do Lumiar",
        name: "Escola Básica do Lumiar",
        service: true,
        due: "Wed",
        what: "Parents’ evening on 22 Oct: choose a 10-minute slot",
        tags: ["Booking form", "6 slots left"],
        quoteFrom: "Secretariat, Friday 16:40",
        quote:
          "Please choose a 10-minute slot for parents’ evening on 22 October between 18:00 and 20:00.",
        context: "You are free after 18:30 that day.",
        buttons: [
          { label: "Book 18:30", variant: "primary", keys: "1" },
          { label: "See all slots", variant: "secondary", shortcut: "list.open" },
        ],
      },
      {
        id: "p3",
        signal: "decide",
        who: "Ginásio Alto",
        name: "Ginásio Alto",
        service: true,
        due: "1 Nov",
        what: "Membership renews on 1 November for €420: keep or cancel?",
        tags: ["11 visits in 3 months", "€420 / year"],
        quoteFrom: "Ginásio Alto, 9 October",
        quote: "Your annual membership renews on 1 November for €420. Reply STOP to cancel.",
        context: "You went 11 times in the last 3 months, about €12.70 a visit.",
        buttons: [
          { label: "Keep it", variant: "primary", keys: "1" },
          { label: "Cancel", variant: "secondary", keys: "2" },
        ],
      },
      {
        id: "p2",
        signal: "pay",
        who: "Luz Energia",
        name: "Luz Energia",
        service: true,
        due: "20 Oct · €62.18",
        what: "Electricity bill for September, by direct debit",
        tags: ["September 2026.pdf", "−8% vs August"],
        quoteFrom: "Luz Energia, 10 October",
        quote: "Your September bill is €62.18, paid by direct debit on 20 October.",
        context: "Direct debit set up. Nothing to do unless you want to check it.",
        buttons: [{ label: "Looks right", variant: "secondary", keys: "1" }],
      },
    ],
  },
};

export interface ConversationData {
  id: string;
  from: string;
  service?: boolean;
  org: string;
  subject: string;
  snippet: string;
  time: string;
  unread: boolean;
  unsure?: boolean;
  note: string;
  sortedLine: string;
  why: string[];
  links: { icon: IconName; title: string; kind: string }[];
  messages: { from: string; time: string; body: string; file?: string }[];
}

const person = (
  id: string,
  from: string,
  org: string,
  subject: string,
  snippet: string,
  time: string,
  body: string,
  note: string,
  unread = false,
): ConversationData => ({
  id,
  from,
  org,
  subject,
  snippet,
  time,
  unread,
  note,
  sortedLine: "Sorted into Primary, 96% sure",
  why: ["You have written to this sender before.", "A person wrote this, not an automated system."],
  links: [],
  messages: [{ from, time, body }],
});

export const STREAMS: {
  id: string;
  name: string;
  count: number;
  conversations: ConversationData[];
}[] = [
  {
    id: "primary",
    name: "Primary",
    count: 12,
    conversations: [
      {
        id: "c1",
        from: "Sofia Pessoa",
        org: "Brightloop",
        subject: "Q4 proposal: can you review it?",
        snippet: "Attached the revised scope. Thursday works for a call.",
        time: "Fri",
        unread: true,
        note: "Sofia is waiting on your review of the Q4 proposal by Thursday. Version 3 moves the timeline two weeks; the price is unchanged.",
        sortedLine: "Sorted into Primary, 98% sure",
        why: [
          "You have replied to Sofia 14 times this year.",
          "The message asks you a direct question with a deadline.",
          "It is not a bulk or automated sender.",
        ],
        links: [
          { icon: "check", title: "Review Q4 proposal", kind: "Task, Thu" },
          { icon: "document", title: "Brightloop-Q4-scope-v3.pdf", kind: "Document" },
          { icon: "event", title: "Held Thu 09:00", kind: "Schedule" },
        ],
        messages: [
          {
            from: "Sofia Pessoa",
            time: "Tue 10:14",
            body: "Hi Ana, here is the first draft of the Q4 proposal. Let me know what you think.",
          },
          {
            from: "Sofia Pessoa",
            time: "Fri 17:42",
            body: "Attached the revised scope. Thursday works for a call if that is easier; otherwise comments in the doc are great.",
            file: "Brightloop-Q4-scope-v3-final-with-appendix-and-pricing-options.pdf",
          },
        ],
      },
      {
        id: "c2",
        from: "Ana Lopes",
        org: "Oficina Criativa",
        subject: "Workshop on 20 October, a few seats left",
        snippet: "Morning session still has room.",
        time: "Mon",
        unread: true,
        unsure: true,
        note: "An invitation to a workshop on 20 October. Campbooks is not sure whether you know this sender.",
        sortedLine: "Sorted into Primary, 61% sure",
        why: [
          "First message from this sender.",
          "Written to you by name, but sent through a mailing tool.",
          "Similar invitations usually go to Updates.",
        ],
        links: [],
        messages: [
          {
            from: "Ana Lopes",
            time: "Mon 08:20",
            body: "Workshop on 20 October: a few seats left for the morning session. Reply if you want one.",
          },
        ],
      },
      {
        id: "c3",
        from: "Marta Duarte",
        org: "Duarte & Pessoa",
        subject: "Office move: which week?",
        snippet: "Two options from the landlord.",
        time: "Sat",
        unread: false,
        note: "Marta needs you to pick a week for the office move. You are in Porto from 16 to 18 November, which clashes with the second option.",
        sortedLine: "Sorted into Primary, 95% sure",
        why: ["Marta is in your workspace.", "The message asks you to choose."],
        links: [{ icon: "event", title: "Porto trip", kind: "Schedule, 16 to 18 Nov" }],
        messages: [
          {
            from: "Marta Duarte",
            time: "Sat 09:30",
            body: "Two options from the landlord: 2 to 6 November or 16 to 20 November. Which one works for you?",
          },
        ],
      },
      person(
        "c4",
        "Joana Reis",
        "Atelier Norte",
        "NDA before Friday’s kickoff",
        "Could you send the NDA back signed before Friday?",
        "08:12",
        "Could you send the NDA back signed before Friday? We cannot share the brand files until then.",
        "Joana needs the signed NDA before Friday. It is unsigned in Documents.",
        true,
      ),
      person(
        "c5",
        "Tiago Mendes",
        "Brightloop",
        "Move Thursday’s call?",
        "Could we do Friday at 11:00 instead? Same link.",
        "09:05",
        "Something came up on Thursday. Could we do Friday at 11:00 instead? Same link.",
        "Tiago wants to move the call to Friday 11:00, which is free.",
        true,
      ),
      person(
        "c6",
        "Rui Matos",
        "Contabilidade RM",
        "VAT Q3: two receipts missing",
        "Still need the two August receipts from Papelaria Sol.",
        "Mon",
        "Still need the two August receipts from Papelaria Sol. Friday at the latest, please.",
        "Your accountant needs two August receipts by Friday. Both are in Documents.",
      ),
      person(
        "c7",
        "Inês Carvalho",
        "Atelier Norte",
        "Re: Logo files",
        "You: Could you send the SVG versions as well?",
        "4 Oct",
        "Thanks for these! Could you send the SVG versions as well?",
        "You asked Inês for the SVG files 8 days ago and have not heard back.",
      ),
    ],
  },
  {
    id: "updates",
    name: "Updates",
    count: 31,
    conversations: [
      {
        id: "u1",
        from: "Cloudhost",
        service: true,
        org: "Service",
        subject: "Scheduled maintenance on 20 October",
        snippet: "Your server restarts between 02:00 and 02:30.",
        time: "Mon",
        unread: true,
        note: "Routine maintenance notice. Nothing for you to do.",
        sortedLine: "Sorted into Updates, 99% sure",
        why: ["Automated sender.", "Informational, no question for you."],
        links: [],
        messages: [
          {
            from: "Cloudhost",
            time: "Mon 08:00",
            body: "Your server will restart between 02:00 and 02:30 on 20 October.",
          },
        ],
      },
    ],
  },
  {
    id: "receipts",
    name: "Receipts",
    count: 8,
    conversations: [
      {
        id: "r1",
        from: "Cloudhost",
        service: true,
        org: "Service",
        subject: "Invoice CH-20418, €148.00",
        snippet: "Due Friday 16 October.",
        time: "12 Oct",
        unread: true,
        note: "An invoice for €148.00, due Friday. It is filed in Documents and will be matched to your bank statement once paid.",
        sortedLine: "Sorted into Receipts, 99% sure",
        why: ["Has an invoice attached.", "Amount and due date found."],
        links: [
          { icon: "document", title: "CH-20418.pdf", kind: "Document" },
          { icon: "check", title: "Pay Cloudhost", kind: "Task, Fri" },
        ],
        messages: [
          {
            from: "Cloudhost",
            time: "12 Oct",
            body: "Your invoice CH-20418 for €148.00 is due on 16 October.",
            file: "CH-20418.pdf",
          },
        ],
      },
    ],
  },
  {
    id: "news",
    name: "Newsletters",
    count: 46,
    conversations: [
      {
        id: "n1",
        from: "The Weekly Ledger",
        service: true,
        org: "Newsletter",
        subject: "Five tax dates before year end",
        snippet: "Plus what changes for freelancers in 2027.",
        time: "Sun",
        unread: false,
        note: "A weekly newsletter. You open about one in four.",
        sortedLine: "Sorted into Newsletters, 99% sure",
        why: ["Sent to a list.", "Has an unsubscribe link."],
        links: [],
        messages: [
          {
            from: "The Weekly Ledger",
            time: "Sun 07:00",
            body: "Five tax dates before year end, plus what changes for freelancers in 2027.",
          },
        ],
      },
    ],
  },
  { id: "clients", name: "Clients", count: 0, conversations: [] },
];
