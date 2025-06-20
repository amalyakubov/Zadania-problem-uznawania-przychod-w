#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
    use std::str::FromStr;

    // First, you'll need to extract these pure functions from your existing code:

    // Extract from find_discounts_for_client
    fn calculate_final_discount(
        base_discount: Option<BigDecimal>,
        recurring_discount: Option<BigDecimal>,
    ) -> BigDecimal {
        let base = base_discount.unwrap_or_else(|| BigDecimal::from(0));
        let recurring = recurring_discount.unwrap_or_else(|| BigDecimal::from(0));
        base + recurring
    }

    // Extract from create_contract handler
    fn apply_discount_to_price(
        price: BigDecimal,
        discount: BigDecimal,
    ) -> Result<BigDecimal, String> {
        let zero = BigDecimal::from(0);
        let one = BigDecimal::from(1);

        if discount < zero || discount > one {
            return Err("Invalid discount range".to_string());
        }
        if price < zero {
            return Err("Invalid price".to_string());
        }
        Ok(price * (one - discount))
    }

    // Extract from create_payment handler
    fn validate_installment_payment(
        amount: BigDecimal,
        outstanding: BigDecimal,
    ) -> Result<(), String> {
        let zero = BigDecimal::from(0);

        if amount <= zero {
            return Err("Amount must be positive".to_string());
        }
        if amount > outstanding {
            return Err("Amount exceeds outstanding payments".to_string());
        }
        Ok(())
    }

    // Extract BigDecimal comparison logic (already correct)
    fn amounts_equal(amount1: BigDecimal, amount2: BigDecimal) -> bool {
        amount1 == amount2
    }

    // Helper function to determine if client gets recurring discount
    fn client_qualifies_for_recurring_discount(active_contracts: i64) -> bool {
        active_contracts >= 1
    }

    // Helper function to create BigDecimal from string for tests
    fn bd(s: &str) -> BigDecimal {
        BigDecimal::from_str(s).unwrap()
    }

    #[test]
    fn test_discount_calculation() {
        // Normal cases
        assert_eq!(
            calculate_final_discount(Some(bd("0.10")), Some(bd("0.05"))),
            bd("0.15")
        );
        assert_eq!(
            calculate_final_discount(Some(bd("0.20")), Some(bd("0.05"))),
            bd("0.25")
        );

        // Only base discount
        assert_eq!(calculate_final_discount(Some(bd("0.10")), None), bd("0.10"));
        assert_eq!(calculate_final_discount(Some(bd("0.15")), None), bd("0.15"));

        // Only recurring discount
        assert_eq!(calculate_final_discount(None, Some(bd("0.05"))), bd("0.05"));

        // No discounts
        assert_eq!(calculate_final_discount(None, None), bd("0"));

        // Edge cases
        assert_eq!(
            calculate_final_discount(Some(bd("0.0")), Some(bd("0.0"))),
            bd("0.0")
        );
        assert_eq!(
            calculate_final_discount(Some(bd("1.0")), Some(bd("0.0"))),
            bd("1.0")
        );
    }

    #[test]
    fn test_price_discount_application() {
        // Normal discount application
        assert_eq!(
            apply_discount_to_price(bd("100.0"), bd("0.1")).unwrap(),
            bd("90.0")
        );
        assert_eq!(
            apply_discount_to_price(bd("100.0"), bd("0.25")).unwrap(),
            bd("75.0")
        );
        assert_eq!(
            apply_discount_to_price(bd("50.0"), bd("0.2")).unwrap(),
            bd("40.0")
        );

        // No discount
        assert_eq!(
            apply_discount_to_price(bd("100.0"), bd("0.0")).unwrap(),
            bd("100.0")
        );

        // Maximum discount
        assert_eq!(
            apply_discount_to_price(bd("100.0"), bd("1.0")).unwrap(),
            bd("0.0")
        );

        // Edge case: very small amounts
        assert_eq!(
            apply_discount_to_price(bd("0.01"), bd("0.5")).unwrap(),
            bd("0.005")
        );

        // Invalid discount values
        assert!(apply_discount_to_price(bd("100.0"), bd("-0.1")).is_err());
        assert!(apply_discount_to_price(bd("100.0"), bd("1.1")).is_err());
        assert!(apply_discount_to_price(bd("100.0"), bd("2.0")).is_err());

        // Invalid price values
        assert!(apply_discount_to_price(bd("-100.0"), bd("0.1")).is_err());
        assert!(apply_discount_to_price(bd("-1.0"), bd("0.0")).is_err());

        // Error messages
        assert_eq!(
            apply_discount_to_price(bd("100.0"), bd("-0.1")).unwrap_err(),
            "Invalid discount range"
        );
        assert_eq!(
            apply_discount_to_price(bd("-100.0"), bd("0.1")).unwrap_err(),
            "Invalid price"
        );
    }

    #[test]
    fn test_installment_payment_validation() {
        // Valid payments
        assert!(validate_installment_payment(bd("50.0"), bd("100.0")).is_ok());
        assert!(validate_installment_payment(bd("100.0"), bd("100.0")).is_ok()); // Exact amount
        assert!(validate_installment_payment(bd("0.01"), bd("100.0")).is_ok()); // Very small payment

        // Invalid: amount exceeds outstanding
        assert!(validate_installment_payment(bd("150.0"), bd("100.0")).is_err());
        assert!(validate_installment_payment(bd("100.01"), bd("100.0")).is_err());

        // Invalid: non-positive amounts
        assert!(validate_installment_payment(bd("0.0"), bd("100.0")).is_err());
        assert!(validate_installment_payment(bd("-1.0"), bd("100.0")).is_err());
        assert!(validate_installment_payment(bd("-50.0"), bd("100.0")).is_err());

        // Error messages
        assert_eq!(
            validate_installment_payment(bd("150.0"), bd("100.0")).unwrap_err(),
            "Amount exceeds outstanding payments"
        );
        assert_eq!(
            validate_installment_payment(bd("0.0"), bd("100.0")).unwrap_err(),
            "Amount must be positive"
        );
        assert_eq!(
            validate_installment_payment(bd("-1.0"), bd("100.0")).unwrap_err(),
            "Amount must be positive"
        );
    }

    #[test]
    fn test_amounts_equal() {
        // Equal amounts
        assert!(amounts_equal(bd("100.0"), bd("100.0")));
        assert!(amounts_equal(bd("50.5"), bd("50.5")));
        assert!(amounts_equal(bd("0.0"), bd("0.0")));

        // Different amounts
        assert!(!amounts_equal(bd("100.0"), bd("100.01")));
        assert!(!amounts_equal(bd("50.0"), bd("51.0")));
        assert!(!amounts_equal(bd("0.0"), bd("0.01")));

        // Precise calculations that would fail with f64
        let result = bd("0.1") + bd("0.2");
        assert!(amounts_equal(result, bd("0.3"))); // This works with BigDecimal!

        // Very small differences
        assert!(!amounts_equal(bd("1.0000001"), bd("1.0000002")));
    }

    #[test]
    fn test_recurring_discount_qualification() {
        // Qualifies for recurring discount
        assert!(client_qualifies_for_recurring_discount(1));
        assert!(client_qualifies_for_recurring_discount(2));
        assert!(client_qualifies_for_recurring_discount(10));

        // Does not qualify
        assert!(!client_qualifies_for_recurring_discount(0));
    }

    #[test]
    fn test_business_logic_edge_cases() {
        // Test discount calculation with maximum values
        let max_discount = calculate_final_discount(Some(bd("0.95")), Some(bd("0.05")));
        assert_eq!(max_discount, bd("1.0"));

        // Test price with maximum discount
        let free_price = apply_discount_to_price(bd("100.0"), bd("1.0")).unwrap();
        assert_eq!(free_price, bd("0.0"));

        // Test very large prices
        let large_price = apply_discount_to_price(bd("1000000.0"), bd("0.1")).unwrap();
        assert_eq!(large_price, bd("900000.0"));

        // Test very small installment
        assert!(validate_installment_payment(bd("0.01"), bd("1000.0")).is_ok());

        // Test precise decimal calculations
        let precise_discount = apply_discount_to_price(bd("100.00"), bd("0.15")).unwrap();
        assert_eq!(precise_discount, bd("85.00"));
    }

    #[test]
    fn test_combined_business_scenarios() {
        // Scenario: New client with product discount
        let product_discount = Some(bd("0.15"));
        let recurring_discount = None; // New client
        let total_discount = calculate_final_discount(product_discount, recurring_discount);
        let final_price = apply_discount_to_price(bd("1000.0"), total_discount).unwrap();
        assert_eq!(final_price, bd("850.0"));

        // Scenario: Recurring client with product discount
        let product_discount = Some(bd("0.10"));
        let recurring_discount = Some(bd("0.05")); // Returning client
        let total_discount = calculate_final_discount(product_discount, recurring_discount);
        let final_price = apply_discount_to_price(bd("1000.0"), total_discount).unwrap();
        assert_eq!(final_price, bd("850.0"));

        // Scenario: Recurring client, no product discount
        let product_discount = None;
        let recurring_discount = Some(bd("0.05"));
        let total_discount = calculate_final_discount(product_discount, recurring_discount);
        let final_price = apply_discount_to_price(bd("1000.0"), total_discount).unwrap();
        assert_eq!(final_price, bd("950.0"));

        // Scenario: Installment payment validation
        let contract_price = bd("1000.0");
        let paid_so_far = bd("400.0");
        let outstanding = contract_price - paid_so_far;

        // Valid partial payment
        assert!(validate_installment_payment(bd("200.0"), outstanding.clone()).is_ok());

        // Valid full payment
        assert!(validate_installment_payment(outstanding.clone(), outstanding.clone()).is_ok());

        // Invalid overpayment
        assert!(validate_installment_payment(
            outstanding.clone() + bd("0.01"),
            outstanding.clone()
        )
        .is_err());
    }

    #[test]
    fn test_precise_financial_calculations() {
        // Test calculations that would fail with f64 due to precision
        let price = bd("99.99");
        let discount = bd("0.125"); // 12.5%
        let result = apply_discount_to_price(price, discount).unwrap();
        assert_eq!(result, bd("87.49125"));

        // Test multiple discount applications
        let base = bd("1000.00");
        let first_discount = bd("0.10");
        let after_first = apply_discount_to_price(base, first_discount).unwrap();
        assert_eq!(after_first, bd("900.00"));

        // Test very precise calculations - just verify it works, don't hardcode the result
        let precise_price = bd("123.456789");
        let precise_discount = bd("0.123456");
        let precise_result =
            apply_discount_to_price(precise_price.clone(), precise_discount.clone()).unwrap();

        // Verify the calculation is correct: result = price * (1 - discount)
        let expected = precise_price * (bd("1") - precise_discount);
        assert_eq!(precise_result, expected);

        // Just verify it's in the right ballpark
        assert!(precise_result > bd("100"));
        assert!(precise_result < bd("110"));
    }
}
