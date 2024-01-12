use crate::models::Partner;
use serenity::builder::CreateSelectMenuOption;

const PARTNER_PAGE_LEN: usize = 20;

/// Gets the highest page number for a partner list
pub fn max_partner_page(partners: &[Partner]) -> usize {
	let mut max_page = partners.len() / PARTNER_PAGE_LEN;
	if partners.len() % PARTNER_PAGE_LEN == 0 {
		max_page = max_page.saturating_sub(1);
	}
	max_page
}

/// Gets the partner list for a particular page number.
/// The `default_selection_id`
pub fn get_partners_for_page(
	partners: &[Partner],
	page_number: usize,
	default_selection_id: &str,
) -> Vec<CreateSelectMenuOption> {
	if partners.is_empty() {
		return Vec::new();
	}

	let mut options: Vec<CreateSelectMenuOption> = Vec::with_capacity(22);
	if page_number > 0 {
		options.push(CreateSelectMenuOption::new("Previous Page", "<"));
	}
	for partner in partners
		.iter()
		.skip(page_number * PARTNER_PAGE_LEN)
		.take(PARTNER_PAGE_LEN)
	{
		let mut option = CreateSelectMenuOption::new(&partner.display_name, &partner.partnership_id);
		if partner.partnership_id == default_selection_id {
			option = option.default_selection(true);
		}
		options.push(option);
	}
	if page_number < max_partner_page(partners) {
		options.push(CreateSelectMenuOption::new("Next Page", ">"));
	}

	options
}
