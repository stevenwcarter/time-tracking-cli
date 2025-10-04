export type Client = {
  uuid: string;
  name: string;
  description: string;
  archived?: boolean;
  createdAt?: string;
  updatedAt?: string;
};
export type Estimate = {
  uuid: string;
  name: string;
  status: string;
  description: string;
  clientUuid: string;
  archived?: boolean;
  createdAt?: string;
  updatedAt?: string;
  client?: Client;
};
export type Section = {
  uuid: string;
  approachUuid: string;
  name: string;
  description: string;
  createdAt?: string;
  updatedAt?: string;
  totals: LineItemTotals;
};
export type Approach = {
  uuid: string;
  name: string;
  description: string;
  estimateUuid: string;
  archived?: boolean;
  createdAt?: string;
  updatedAt?: string;
  totalsByFeature?: FeatureTotals[];
  client?: Client;
  estimate?: Estimate;
  totalHours?: LineItemTotals;
};
export type LineItem = {
  uuid: string;
  sectionUuid: string;
  task?: string;
  notes?: string;
  feature?: string;
  beLow: number;
  beHigh: number;
  feLow: number;
  feHigh: number;
  quantity: number;
  createdAt?: string;
  updatedAt?: string;
};
export type LineItemTotals = {
  beLow: number;
  beHigh: number;
  feLow: number;
  feHigh: number;
};
export type FeatureTotals = {
  feature: string;
  beLow: number;
  beHigh: number;
  feLow: number;
  feHigh: number;
};
