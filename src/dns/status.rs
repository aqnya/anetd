use std::io;
use tokio::io::{AsyncWrite, AsyncWriteExt};

// see https://cs.android.com/android/platform/superproject/+/android-latest-release:packages/modules/Connectivity/staticlibs/netd/libnetdutils/include/netdutils/ResponseCode.h
#[allow(dead_code)]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsProxyStatus {
    ActionInitiated = 100,
    InterfaceListResult = 110,
    TetherInterfaceListResult = 111,
    TetherDnsFwdTgtListResult = 112,
    TtyListResult = 113,
    TetheringStatsListResult = 114,
    TetherDnsFwdNetIdResult = 115,

    CommandOkay = 200,
    TetherStatusResult = 210,
    IpFwdStatusResult = 211,
    InterfaceGetCfgResult = 213,
    UsbRNDISStatusResult = 215,
    InterfaceRxCounterResult = 216,
    InterfaceTxCounterResult = 217,
    InterfaceRxThrottleResult = 218,
    InterfaceTxThrottleResult = 219,
    QuotaCounterResult = 220,
    TetheringStatsResult = 221,
    DnsProxyQueryResult = 222,
    ClatdStatusResult = 223,

    OperationFailed = 400,
    DnsProxyOperationFailed = 401,
    ServiceStartFailed = 402,
    ServiceStopFailed = 403,

    CommandSyntaxError = 500,
    CommandParameterError = 501,

    InterfaceChange = 600,
    BandwidthControl = 601,
    ServiceDiscoveryFailed = 602,
    ServiceDiscoveryServiceAdded = 603,
    ServiceDiscoveryServiceRemoved = 604,
    ServiceRegistrationFailed = 605,
    ServiceRegistrationSucceeded = 606,
    ServiceResolveFailed = 607,
    ServiceResolveSuccess = 608,
    ServiceSetHostnameFailed = 609,
    ServiceSetHostnameSuccess = 610,
    ServiceGetAddrInfoFailed = 611,
    ServiceGetAddrInfoSuccess = 612,
    InterfaceClassActivity = 613,
    InterfaceAddressChange = 614,
    InterfaceDnsInfo = 615,
    RouteChange = 616,
    StrictCleartext = 617,
}

impl DnsProxyStatus {
    #[inline]
    pub const fn code(self) -> u16 {
        self as u16
    }

    #[inline]
    pub async fn write<W: AsyncWrite + Unpin>(self, w: &mut W) -> io::Result<()> {
        let code = self.code();

        let buf = [
            b'0' + ((code / 100) % 10) as u8,
            b'0' + ((code / 10) % 10) as u8,
            b'0' + (code % 10) as u8,
            0,
        ];

        w.write_all(&buf).await
    }
}
