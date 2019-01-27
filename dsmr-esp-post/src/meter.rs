#[repr(C,packed)]
pub struct UsageData {
    timestamp_year: u8,
    timestamp_rest: u32,
    pub power_delivered: u32,
    pub power_returned: u32,
    pub energy_delivered_tariff1: u32,
    pub energy_delivered_tariff2: u32,
    pub energy_returned_tariff1: u32,
    pub energy_returned_tariff2: u32,
    pub power_delivered_l1: u32,
    pub power_delivered_l2: u32,
    pub power_delivered_l3: u32,
    gas_timestamp_year: u8,
    gas_timestamp_rest: u32,
    pub gas_delivered: u32,
}

pub const USAGEDATA_SIZE: usize = 50;

impl UsageData {
    fn to_tm(year: u8, rest: u32) -> Result<time::Tm, time::ParseError> {
        let mut rest_str = rest.to_string();
        if rest_str.len() < 10 {
            rest_str = format!("0{}", rest_str);
        }
        let time_str = format!("20{}{}", year.to_string(), rest_str);
        let mut time = time::strptime(&time_str, "%Y%m%d%H%M%S")?;
        // this may generate a few datapoints in the wrong timezone but we can live
        // with that
        time.tm_utcoff = time::now().tm_utcoff;
        time.tm_isdst = time::now().tm_isdst;
        Ok(time)
    }

    pub fn energy_timestamp(&self) -> Result<time::Tm, time::ParseError> {
        Self::to_tm(self.timestamp_year, self.timestamp_rest)
    }

    pub fn gas_timestamp(&self) -> Result<time::Tm, time::ParseError> {
        Self::to_tm(self.gas_timestamp_year, self.gas_timestamp_rest)
    }

}
