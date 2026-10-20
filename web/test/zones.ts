// Zones a device may be in (west and east of UTC, with daylight saving), and a check run with this device set to each and to UTC.

export const ZONES = ["America/New_York", "Europe/London", "Europe/Berlin", "UTC"] as const

export const everywhere = (check: (zone: string) => void): void => {
  const was = process.env.TZ
  try {
    for (const zone of ZONES) {
      process.env.TZ = zone
      check(zone)
    }
  } finally {
    process.env.TZ = was
  }
}
