
#include "stdafx.h"

#include "gucci.h"

#include "app/app.h"
#include "app/globals.h"
#include "game/data/data.h"



/*	===========================================================================
	Uplink ratings

	Used in : Rating 


								Rating name		    Score 
								-----------         -----					*/


const UplinkRating 
UPLINKRATING [] =  {			
								"Unregistered",		0,      
                                "Registered",       1,
								"Beginner",			2,     
								"Novice",			6,     
								"Confident",		15,    
                                "Intermediate",		35,    
								"Skilled",			60,    
								"Experienced",		90,    
								"Knowledgeable",	125,   
								"Uber-Skilled",		160,   
								"Professional",		220,   
								"Elite",			300,   
								"Mage",				400,   
								"Expert",			600,   
								"Veteren",			1000,  
								"Techno-mage",		1500,  
								"TERMINAL",			2500   
																	};



const UplinkRating 
NEUROMANCERRATING [] = {		"Morally bankrupt",	-1024,
								"Sociopathic",  	-512,
								"Indiscriminate",	-128,
								"Notorious",		-32,
								"Untrustworthy",	-16,
								"Neutral",  		0,
								"Aggressive",		32,
								"Single minded",	64,
								"Activist", 		256,
								"Anarchic",     	1024,
								"Revolutionary",	2048,
																	};


const int 
NEUROMANCERCHANGE [] = {		
								0,					// Special
								0,					// Steal file
								5,					// Destroy file
								0,					// Find data
							   -5,					// Change data
							  -20,					// Frame user
							  -30,					// Trace user
							  -10,					// Change account
							   30,					// Remove computer
							   60,					// Remove company
							  -60					// Remove user
	
																	};

/*  ===========================================================================
	Physical locations of gateways

									City						Country				X		Y
									----						-------				-		-
  */


const PhysicalGatewayLocation 
PHYSICALGATEWAYLOCATIONS [] = {


									"London",					"United Kingdom",	282,		69,
									"Tokyo",					"Japan",			514,		104,
									"Los Angeles",				"USA",				91,			125,
									"New York",					"USA",				171,		98,
									"Chicago",					"USA",				138,		88,
									"Moscow",					"Russia",			331,		70,
									"Hong Kong",				"China",			485,		133,
									"Sydney",					"Australia",		547,		244

																	};

const PhysicalGatewayLocation 
PHYSICALGATEWAYLOCATIONS_DEFCON [] = {


									"London",					"United Kingdom",	295,		75,
									"Tokyo",					"Japan",			527,		103,
									"Los Angeles",				"USA",				99,			106,
									"New York",					"USA",				175,		94,
									"Chicago",					"USA",				151,		91,
									"Moscow",					"Russia",			356,		65,
									"Hong Kong",				"China",			483,		126,
									"Sydney",					"Australia",		545,		226

																	};
