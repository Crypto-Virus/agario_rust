
use std::str::FromStr;
use ethers::prelude::Signature;



 pub fn authenticate(address: &str, signature: &str) -> bool {
    match recover_public_address(signature) {
        Ok(address_) => if address.to_lowercase() == address_ {true} else {false}
        Err(_) => false
    }
}


fn recover_public_address(signature: &str) -> crate::Result<String> {
    let sig = Signature::from_str(signature)?;
    let address = sig.recover("let's play")?;
    let address = format!("{:#x}", address);
    Ok(address)
}
